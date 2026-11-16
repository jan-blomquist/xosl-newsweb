//! The command line: its settings, and the `run` command.
//!
//! Each setting is a flag and an environment variable, defined once here;
//! `--help` lists them.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::builder::BoolishValueParser;
use clap::{Args, Parser, Subcommand};
use tokio_util::sync::CancellationToken;
use tonic::service::Routes;
use url::Url;

use crate::client::{self, ClientError, Format, ListQuery};
use crate::clock::{Clock, SystemClock};
use crate::exchange::client::{Client, Exchange, HttpTransport};
use crate::files::{Files, FilesError};
use crate::health::{self, CHECK_TIMEOUT, EVALUATION_INTERVAL, ServeError};
use crate::ingest::{self, JobSettings, Jobs};
use crate::logging::{self, InitError, LogFormat};
use crate::pb::newsweb_service_server::NewswebServiceServer;
use crate::server::{MirrorService, StatusSettings};
use crate::store::{Store, StoreError};
use crate::webhook::{self, HttpPost, WebhookSettings};

/// The database file, in the data folder.
const DATABASE_FILE: &str = "mirror.sqlite";
/// The folder of the attachments' bytes, in the data folder.
const ATTACHMENTS_FOLDER: &str = "attachments";

/// A self-hosted mirror of Oslo Børs NewsWeb. Unofficial.
#[derive(Debug, Parser)]
#[command(name = "xosl-newsweb", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Opens the mirror's data folder and, with --ingest, copies NewsWeb into
    /// it until SIGINT or SIGTERM.
    Run(RunSettings),

    /// Asks a running mirror whether it is healthy. Prints `healthy`,
    /// `unhealthy` or `unreachable: <cause>`; exits with 0 only when healthy.
    Health(Server),

    /// Prints the disclosures in a time range, newest first.
    List(ListSettings),

    /// Prints one disclosure.
    Get(GetSettings),

    /// Writes an attachment's bytes to a file, or to stdout.
    Attachment(AttachmentSettings),

    /// Prints events as they are written, until interrupted.
    Watch(WatchSettings),

    /// Prints what the mirror holds and how its ingest is doing.
    Status(DataSettings),

    /// Prints the failure log, newest first.
    Failures(FailuresSettings),
}

/// The mirror a client command calls.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct Server {
    /// The address of the mirror's gRPC server.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_ADDRESS",
        value_name = "URL",
        default_value = "http://127.0.0.1:50051"
    )]
    pub address: String,
}

/// The settings every data command has.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct DataSettings {
    #[command(flatten)]
    pub server: Server,

    /// Print one JSON object per line instead of text.
    #[arg(long)]
    pub json: bool,
}

impl DataSettings {
    fn format(&self) -> Format {
        if self.json {
            Format::Json
        } else {
            Format::Text
        }
    }
}

/// The settings of `list`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ListSettings {
    #[command(flatten)]
    pub data: DataSettings,

    /// Only disclosures published at or after this time: an RFC 3339
    /// timestamp, or a date, which is the start of that day in Oslo.
    #[arg(long, value_name = "TIME", value_parser = time_argument)]
    pub from: Option<i64>,

    /// Only disclosures published before this time; same formats.
    #[arg(long, value_name = "TIME", value_parser = time_argument)]
    pub before: Option<i64>,

    /// Only this issuer sign (ticker); repeat for several.
    #[arg(long = "sign", value_name = "SIGN")]
    pub signs: Vec<String>,

    /// Stop after this many disclosures.
    #[arg(long, value_name = "N", value_parser = at_least(1_u64))]
    pub limit: Option<u64>,
}

impl ListSettings {
    fn query(&self) -> ListQuery {
        ListQuery {
            from_ns: self.from.unwrap_or(0),
            before_ns: self.before.unwrap_or(0),
            signs: self.signs.clone(),
            limit: self.limit,
        }
    }
}

/// The settings of `get`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct GetSettings {
    #[command(flatten)]
    pub data: DataSettings,

    /// The exchange's message id: the number in the disclosure's address.
    #[arg(value_name = "MESSAGE_ID")]
    pub message_id: i64,
}

/// The settings of `attachment`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AttachmentSettings {
    #[command(flatten)]
    pub server: Server,

    /// The disclosure the attachment belongs to.
    #[arg(value_name = "MESSAGE_ID")]
    pub message_id: i64,

    #[arg(value_name = "ATTACHMENT_ID")]
    pub attachment_id: i64,

    /// Write the bytes to this file instead of stdout.
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// The settings of `watch`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WatchSettings {
    #[command(flatten)]
    pub data: DataSettings,

    /// The `seq` of the last event already handled; 0 replays the whole log.
    /// Default: the latest event, so only new events are printed.
    #[arg(long, value_name = "SEQ", value_parser = clap::value_parser!(i64).range(0..))]
    pub since: Option<i64>,
}

/// The settings of `failures`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct FailuresSettings {
    #[command(flatten)]
    pub data: DataSettings,

    /// Only the failures of this disclosure and its attachments.
    #[arg(long, value_name = "ID", value_parser = clap::value_parser!(i64).range(1..))]
    pub message_id: Option<i64>,

    /// Stop after this many failures.
    #[arg(long, value_name = "N", value_parser = at_least(1_u64))]
    pub limit: Option<u64>,
}

fn time_argument(text: &str) -> Result<i64, String> {
    client::parse_time(text).map_err(|error| error.to_string())
}

/// The settings of `run`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct RunSettings {
    /// The folder of the database and the attachments; created when missing.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_DATA_DIR",
        value_name = "DIR",
        default_value = "./data"
    )]
    pub data_dir: PathBuf,

    /// Poll NewsWeb, backfill past days, read disclosures in full and
    /// download attachments. Off: no request reaches the exchange.
    #[arg(long, env = "XOSL_NEWSWEB_INGEST", value_parser = BoolishValueParser::new())]
    pub ingest: bool,

    /// The shortest time between two requests to the exchange, in
    /// milliseconds; at least 1000.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_REQUEST_INTERVAL_MS",
        value_name = "N",
        default_value_t = 1000,
        value_parser = at_least(1000_u64)
    )]
    pub request_interval_ms: u64,

    /// How often today is listed, in seconds; also how long a job with no
    /// work waits before it looks again. At least 1.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_POLL_INTERVAL_SECS",
        value_name = "N",
        default_value_t = 60,
        value_parser = at_least(1_u64)
    )]
    pub poll_interval_secs: u64,

    /// How many past days are kept listed and checked for gaps; at least 1
    /// and at most 36500. The exchange's history starts in 1998.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_LOOKBACK_DAYS",
        value_name = "N",
        default_value_t = 7,
        value_parser = lookback_days
    )]
    pub lookback_days: u32,

    /// Sends each event to this URL as an HTTP POST, in order, from the
    /// moment it is first set. http or https. No webhook when unset.
    #[arg(long, env = "XOSL_NEWSWEB_WEBHOOK_URL", value_name = "URL", value_parser = webhook_url)]
    pub webhook_url: Option<Url>,

    /// Signs each webhook request with HMAC-SHA256 under this secret.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_WEBHOOK_SECRET",
        value_name = "SECRET",
        hide_env_values = true,
        requires = "webhook_url"
    )]
    pub webhook_secret: Option<String>,

    /// The address of the gRPC server, host and port.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_LISTEN",
        value_name = "ADDRESS",
        default_value = "127.0.0.1:50051"
    )]
    pub listen: SocketAddr,

    /// How log lines are written to stderr: text, or one JSON object per
    /// line. The level is set by RUST_LOG; the default is info.
    #[arg(
        long,
        env = "XOSL_NEWSWEB_LOG_FORMAT",
        value_name = "FORMAT",
        value_enum,
        default_value_t
    )]
    pub log_format: LogFormat,
}

/// The largest `--lookback-days`: a hundred years. A larger value is a
/// mistake, and one pass of the backfill would send a request for each day.
const MAX_LOOKBACK_DAYS: u32 = 36_500;

/// The delay before the second try of a failed webhook delivery.
const WEBHOOK_FIRST_RETRY_DELAY: Duration = Duration::from_secs(1);

fn webhook_url(value: &str) -> Result<Url, String> {
    Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .ok_or_else(|| "must be an http or https URL".to_owned())
}

fn lookback_days(value: &str) -> Result<u32, String> {
    let range = 1..=MAX_LOOKBACK_DAYS;
    value
        .parse()
        .ok()
        .filter(|days| range.contains(days))
        .ok_or_else(|| format!("must be at least 1 and at most {MAX_LOOKBACK_DAYS}"))
}

impl RunSettings {
    pub fn request_interval(&self) -> Duration {
        Duration::from_millis(self.request_interval_ms)
    }

    /// The settings of the webhook; `None` when no URL is set.
    pub fn webhook_settings(&self) -> Option<WebhookSettings> {
        let url = self.webhook_url.as_ref()?;
        Some(WebhookSettings {
            url: url.to_string(),
            secret: self.webhook_secret.clone(),
            first_retry_delay: WEBHOOK_FIRST_RETRY_DELAY,
        })
    }

    pub fn mirror_settings(&self) -> MirrorSettings {
        MirrorSettings {
            ingest: self.ingest,
            jobs: JobSettings {
                poll_interval: Duration::from_secs(self.poll_interval_secs),
                lookback_days: self.lookback_days,
            },
        }
    }
}

/// A value parser for a whole number of at least `min`.
fn at_least<T>(min: T) -> impl Fn(&str) -> Result<T, String> + Clone + Send + Sync + 'static
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy + Send + Sync + 'static,
{
    move |value| {
        let number: T = value
            .parse()
            .map_err(|_| format!("{value:?} is not a whole number"))?;
        if number < min {
            return Err(format!("must be at least {min}"));
        }
        Ok(number)
    }
}

/// What `run_mirror` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirrorSettings {
    /// Start the ingest jobs.
    pub ingest: bool,
    pub jobs: JobSettings,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Logging(InitError),
    #[error("cannot create the data folder {path:?}")]
    DataDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the data folder {path:?} has no file URL")]
    AttachmentsUrl { path: PathBuf },
    #[error("cannot open the database")]
    Store(#[from] StoreError),
    #[error("cannot open the attachments")]
    Files(#[from] FilesError),
    #[error("cannot create the HTTP client")]
    Http(#[source] reqwest::Error),
    #[error("cannot listen on {address}")]
    Listen {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Serve(#[from] ServeError),
    #[error("cannot listen for SIGINT and SIGTERM")]
    Signal(#[source] std::io::Error),
}

/// The entry point of the binary.
pub async fn main() -> ExitCode {
    let cli = Cli::parse();
    let settings = match cli.command {
        Command::Run(settings) => settings,
        Command::Health(server) => {
            let outcome = health::check(&server.address, CHECK_TIMEOUT).await;
            println!("{outcome}");
            return ExitCode::from(outcome.exit_code());
        }
        command => return exit_code_of(run_client(command).await),
    };
    let result = run(&settings).await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Any other error is already in the log.
            if let RunError::Logging(error) = &error {
                eprintln!("error: {}", logging::chain(error));
            }
            ExitCode::FAILURE
        }
    }
}

/// Prints an error on stderr, in one line, unless the reader of stdout has
/// gone away.
fn exit_code_of(result: Result<(), ClientError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.is_closed_output() => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// A client command: one call to the mirror, printed on stdout.
async fn run_client(command: Command) -> Result<(), ClientError> {
    let mut stdout = std::io::stdout();
    match command {
        Command::List(settings) => {
            let address = &settings.data.server.address;
            let format = settings.data.format();
            client::list(address, &settings.query(), format, &mut stdout).await
        }
        Command::Get(settings) => {
            let address = &settings.data.server.address;
            client::get(
                address,
                settings.message_id,
                settings.data.format(),
                &mut stdout,
            )
            .await
        }
        Command::Attachment(settings) => {
            let address = &settings.server.address;
            let ids = (settings.message_id, settings.attachment_id);
            match &settings.output {
                Some(path) => client::attachment(address, ids.0, ids.1, || create_file(path)).await,
                None => client::attachment(address, ids.0, ids.1, || Ok(stdout)).await,
            }
        }
        Command::Watch(settings) => {
            let stop = CancellationToken::new();
            let interrupted = stop.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    interrupted.cancel();
                }
            });
            let address = &settings.data.server.address;
            let format = settings.data.format();
            client::watch(address, settings.since, format, &mut stdout, &stop).await
        }
        Command::Status(settings) => {
            let format = settings.format();
            client::status(&settings.server.address, format, &mut stdout).await
        }
        Command::Failures(settings) => {
            let address = &settings.data.server.address;
            let message_id = settings.message_id.unwrap_or(0);
            let format = settings.data.format();
            client::failures(address, message_id, settings.limit, format, &mut stdout).await
        }
        Command::Run(_) | Command::Health(_) => Ok(()),
    }
}

/// Creates the output file; an error names the path.
fn create_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::create(path)
        .map_err(|error| std::io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

/// The `run` command: sets up logging, opens the store and the files, and
/// runs the mirror until SIGINT or SIGTERM. A failure after logging is set
/// up is logged at `error` and returned.
pub async fn run(settings: &RunSettings) -> Result<(), RunError> {
    logging::init(settings.log_format).map_err(RunError::Logging)?;
    log_start(settings);
    let result = run_logged(settings).await;
    if let Err(error) = &result {
        tracing::error!(error = %logging::chain(error), "the mirror stopped");
    }
    result
}

/// The `info` line with the version and every setting.
fn log_start(settings: &RunSettings) {
    let data_dir =
        std::path::absolute(&settings.data_dir).unwrap_or_else(|_| settings.data_dir.clone());
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %data_dir.display(),
        listen = %settings.listen,
        ingest = settings.ingest,
        request_interval_ms = settings.request_interval_ms,
        poll_interval_secs = settings.poll_interval_secs,
        lookback_days = settings.lookback_days,
        log_format = %settings.log_format,
        webhook = settings.webhook_url.is_some(),
        "starting"
    );
}

async fn run_logged(settings: &RunSettings) -> Result<(), RunError> {
    let layout = open_data_dir(&settings.data_dir)?;
    let store = Arc::new(Store::open(&layout.database).await?);
    let files = Arc::new(Files::open(layout.attachments.as_str(), [])?);
    // Creating the client sends no request; with ingest off it is not used.
    let transport = HttpTransport::new().map_err(RunError::Http)?;
    let client = Client::new(transport, settings.request_interval());
    let jobs = Jobs::new(&store, &files, &client, &SystemClock);
    let listener = tokio::net::TcpListener::bind(settings.listen)
        .await
        .map_err(|source| RunError::Listen {
            address: settings.listen,
            source,
        })?;

    let mirror_settings = settings.mirror_settings();
    let service = MirrorService::new(
        Arc::clone(&store),
        Arc::clone(&files),
        Arc::clone(jobs.states()),
        SystemClock,
        StatusSettings {
            ingest_enabled: mirror_settings.ingest,
            lookback_days: mirror_settings.jobs.lookback_days,
        },
    );
    let services = Routes::new(NewswebServiceServer::new(service));
    let started_at_ns = SystemClock.now_ns();
    let states = Arc::clone(jobs.states());
    let evaluate = move || {
        health::health(
            &states.snapshot(),
            mirror_settings.ingest,
            SystemClock.now_ns(),
            started_at_ns,
            mirror_settings.jobs.poll_interval,
        )
    };

    // Either part ending, with an error or not, stops the other.
    let shutdown = CancellationToken::new();
    let mirror = async {
        let result = run_mirror(&jobs, &mirror_settings, &shutdown).await;
        shutdown.cancel();
        result.map_err(RunError::from)
    };
    let server = async {
        let result =
            health::serve(listener, services, evaluate, EVALUATION_INTERVAL, &shutdown).await;
        shutdown.cancel();
        result.map_err(RunError::from)
    };
    let post = HttpPost::new().map_err(RunError::Http)?;
    let webhook = async {
        if let Some(settings) = settings.webhook_settings() {
            webhook::run(Arc::clone(&store), post, settings, &shutdown).await;
        }
    };
    let both = async {
        let (mirror, server, ()) = tokio::join!(mirror, server, webhook);
        mirror.and(server)
    };
    tokio::pin!(both);
    tokio::select! {
        result = &mut both => return result,
        signal = shutdown_signal() => signal.map_err(RunError::Signal)?,
    }
    tracing::info!("stopping");
    shutdown.cancel();
    both.await?;
    tracing::info!("stopped");
    Ok(())
}

/// What `run` does once the store and the files are open: logs the coverage
/// of the lookback range, then runs the ingest jobs, when ingest is on,
/// until `shutdown` is cancelled. Returns early only when the coverage
/// cannot be read.
pub async fn run_mirror<E: Exchange, C: Clock>(
    jobs: &Jobs<'_, E, C>,
    settings: &MirrorSettings,
    shutdown: &CancellationToken,
) -> Result<(), StoreError> {
    let lookback_days = settings.jobs.lookback_days;
    ingest::log_coverage(jobs.store, jobs.clock.now_ns(), lookback_days).await?;
    if settings.ingest {
        tracing::info!("ingest is on");
        jobs.run(&settings.jobs, shutdown).await;
    } else {
        tracing::info!("ingest is off: no request goes to the exchange");
        shutdown.cancelled().await;
    }
    Ok(())
}

/// Where the data folder keeps the database and the attachments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    pub database: PathBuf,
    /// A `file` URL with a trailing slash.
    pub attachments: Url,
}

/// The layout of the data folder at `absolute_dir`.
pub fn data_layout(absolute_dir: &Path) -> Result<DataLayout, RunError> {
    let attachments_dir = absolute_dir.join(ATTACHMENTS_FOLDER);
    let attachments =
        Url::from_directory_path(&attachments_dir).map_err(|()| RunError::AttachmentsUrl {
            path: attachments_dir.clone(),
        })?;
    Ok(DataLayout {
        database: absolute_dir.join(DATABASE_FILE),
        attachments,
    })
}

/// Creates the data folder when missing, and returns its layout.
fn open_data_dir(dir: &Path) -> Result<DataLayout, RunError> {
    let data_dir_error = |source| RunError::DataDir {
        path: dir.to_owned(),
        source,
    };
    std::fs::create_dir_all(dir).map_err(data_dir_error)?;
    let absolute_dir = std::fs::canonicalize(dir).map_err(data_dir_error)?;
    data_layout(&absolute_dir)
}

/// Waits for SIGINT or SIGTERM.
async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    use super::*;

    fn parse(args: &[&str]) -> Result<RunSettings, clap::Error> {
        let args = ["xosl-newsweb", "run"].iter().chain(args);
        match Cli::try_parse_from(args)?.command {
            Command::Run(settings) => Ok(settings),
            _ => unreachable!("the arguments start with `run`"),
        }
    }

    fn parse_health(args: &[&str]) -> Result<Server, clap::Error> {
        let args = ["xosl-newsweb", "health"].iter().chain(args);
        match Cli::try_parse_from(args)?.command {
            Command::Health(server) => Ok(server),
            _ => unreachable!("the arguments start with `health`"),
        }
    }

    fn parse_command(args: &[&str]) -> Result<Command, clap::Error> {
        let args = ["xosl-newsweb"].iter().chain(args);
        Ok(Cli::try_parse_from(args)?.command)
    }

    #[test]
    fn cli__definition__passes_clap_checks() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse__no_flags__returns_the_defaults() {
        let settings = parse(&[]).unwrap();

        let expected = RunSettings {
            data_dir: PathBuf::from("./data"),
            ingest: false,
            request_interval_ms: 1000,
            poll_interval_secs: 60,
            lookback_days: 7,
            listen: "127.0.0.1:50051".parse().unwrap(),
            log_format: LogFormat::Text,
            webhook_url: None,
            webhook_secret: None,
        };
        assert_eq!(settings, expected);
    }

    #[test]
    fn parse__every_flag__returns_its_value() {
        let args = [
            "--data-dir",
            "/srv/mirror",
            "--ingest",
            "--request-interval-ms",
            "1500",
            "--poll-interval-secs",
            "5",
            "--lookback-days",
            "30",
            "--listen",
            "0.0.0.0:6000",
            "--log-format",
            "json",
        ];

        let settings = parse(&args).unwrap();

        let expected = RunSettings {
            data_dir: PathBuf::from("/srv/mirror"),
            ingest: true,
            request_interval_ms: 1500,
            poll_interval_secs: 5,
            lookback_days: 30,
            listen: "0.0.0.0:6000".parse().unwrap(),
            log_format: LogFormat::Json,
            webhook_url: None,
            webhook_secret: None,
        };
        assert_eq!(settings, expected);
    }

    #[test]
    fn parse__webhook_url_and_secret__returns_the_webhook_settings() {
        let settings = parse(&[
            "--webhook-url",
            "https://hooks.example/newsweb",
            "--webhook-secret",
            "s3cret",
        ])
        .unwrap();

        let webhook = settings.webhook_settings().unwrap();

        assert_eq!(webhook.url, "https://hooks.example/newsweb");
        assert_eq!(webhook.secret.as_deref(), Some("s3cret"));
    }

    #[test]
    fn parse__no_webhook_url__has_no_webhook_settings() {
        let settings = parse(&[]).unwrap();

        let webhook = settings.webhook_settings();

        assert!(webhook.is_none());
    }

    #[test]
    fn parse__webhook_url_not_http__returns_error_naming_the_setting() {
        let message = parse(&["--webhook-url", "ftp://hooks.example"])
            .unwrap_err()
            .to_string();

        assert!(message.contains("--webhook-url"), "message: {message}");
    }

    #[test]
    fn parse__webhook_secret_without_a_url__returns_error() {
        let result = parse(&["--webhook-secret", "s3cret"]);

        assert!(result.is_err());
    }

    #[test]
    fn parse__smallest_allowed_values__accepts_them() {
        let args = [
            "--request-interval-ms",
            "1000",
            "--poll-interval-secs",
            "1",
            "--lookback-days",
            "1",
        ];

        let settings = parse(&args);

        assert!(settings.is_ok());
    }

    #[test]
    fn parse__lookback_days_above_the_maximum__returns_error_naming_the_maximum() {
        let message = parse(&["--lookback-days", "36501"])
            .unwrap_err()
            .to_string();

        assert!(message.contains("at most 36500"), "message: {message}");
    }

    #[test]
    fn parse__value_below_the_minimum__returns_error_naming_the_setting_and_minimum() {
        let cases = [
            ("--request-interval-ms", "999", "at least 1000"),
            ("--poll-interval-secs", "0", "at least 1"),
            ("--lookback-days", "0", "at least 1"),
        ];
        for (flag, value, minimum) in cases {
            let message = parse(&[flag, value]).unwrap_err().to_string();
            assert!(
                message.contains(flag) && message.contains(minimum),
                "flag: {flag}, message: {message}"
            );
        }
    }

    #[test]
    fn parse__listen_not_an_address__returns_error_naming_the_setting() {
        let message = parse(&["--listen", "everywhere"]).unwrap_err().to_string();

        assert!(message.contains("--listen"), "message: {message}");
    }

    #[test]
    fn parse_health__no_flags__returns_the_default_address() {
        let settings = parse_health(&[]).unwrap();

        assert_eq!(settings.address, "http://127.0.0.1:50051");
    }

    #[test]
    fn parse_health__address_flag__returns_it() {
        let settings = parse_health(&["--address", "http://mirror:6000"]).unwrap();

        assert_eq!(settings.address, "http://mirror:6000");
    }

    #[test]
    fn parse__value_not_a_number__returns_error_naming_the_setting() {
        let message = parse(&["--lookback-days", "week"]).unwrap_err().to_string();

        assert!(message.contains("--lookback-days"), "message: {message}");
    }

    #[test]
    fn try_parse_from__version_flag__displays_name_and_version() {
        let error = Cli::try_parse_from(["xosl-newsweb", "--version"]).unwrap_err();

        let expected = format!("xosl-newsweb {}\n", env!("CARGO_PKG_VERSION"));
        assert_eq!(
            (error.kind(), error.to_string()),
            (ErrorKind::DisplayVersion, expected)
        );
    }

    #[test]
    fn mirror_settings__run_settings__converts_the_units() {
        let settings = parse(&["--poll-interval-secs", "5", "--lookback-days", "30"]).unwrap();

        let mirror = settings.mirror_settings();

        let expected = JobSettings {
            poll_interval: Duration::from_secs(5),
            lookback_days: 30,
        };
        assert_eq!(mirror.jobs, expected);
    }

    #[test]
    fn data_layout__absolute_folder__puts_database_and_attachments_in_it() {
        let layout = data_layout(Path::new("/srv/mirror")).unwrap();

        let expected = DataLayout {
            database: PathBuf::from("/srv/mirror/mirror.sqlite"),
            attachments: Url::parse("file:///srv/mirror/attachments/").unwrap(),
        };
        assert_eq!(layout, expected);
    }

    #[test]
    fn data_layout__relative_folder__returns_error_naming_it() {
        let error = data_layout(Path::new("data")).unwrap_err();

        assert!(matches!(error, RunError::AttachmentsUrl { .. }));
    }

    const DEFAULT_ADDRESS: &str = "http://127.0.0.1:50051";

    #[test]
    fn parse_command__list_without_flags__is_unbounded_with_text_output() {
        let command = parse_command(&["list"]).unwrap();

        let Command::List(settings) = command else {
            panic!("not list: {command:?}");
        };
        let expected = ListQuery::default();
        assert_eq!(
            (
                settings.query(),
                settings.data.json,
                settings.data.server.address
            ),
            (expected, false, DEFAULT_ADDRESS.to_owned())
        );
    }

    #[test]
    fn parse_command__list_with_every_flag__returns_the_query() {
        let args = [
            "list",
            "--from",
            "2026-11-16T08:00:00Z",
            "--before",
            "2026-11-17T08:00:00Z",
            "--sign",
            "EXSH",
            "--sign",
            "ABC",
            "--limit",
            "5",
            "--json",
            "--address",
            "http://mirror:6000",
        ];

        let command = parse_command(&args).unwrap();

        let Command::List(settings) = command else {
            panic!("not list: {command:?}");
        };
        let expected = ListQuery {
            from_ns: 1_794_816_000_000_000_000,
            before_ns: 1_794_902_400_000_000_000,
            signs: vec!["EXSH".into(), "ABC".into()],
            limit: Some(5),
        };
        assert_eq!(settings.query(), expected);
    }

    #[test]
    fn parse_command__list_json_and_address__are_set() {
        let command = parse_command(&["list", "--json", "--address", "http://mirror:6000"]);

        let Ok(Command::List(settings)) = command else {
            panic!("not list: {command:?}");
        };
        assert_eq!(
            (settings.data.json, settings.data.server.address.as_str()),
            (true, "http://mirror:6000")
        );
    }

    #[test]
    fn parse_command__list_from_not_a_time__returns_error_naming_the_flag() {
        let message = parse_command(&["list", "--from", "yesterday"])
            .unwrap_err()
            .to_string();

        assert!(message.contains("--from"), "message: {message}");
    }

    #[test]
    fn parse_command__list_limit_zero__returns_error() {
        let message = parse_command(&["list", "--limit", "0"])
            .unwrap_err()
            .to_string();

        assert!(message.contains("at least 1"), "message: {message}");
    }

    #[test]
    fn parse_command__get__takes_the_message_id() {
        let command = parse_command(&["get", "654321"]).unwrap();

        let Command::Get(settings) = command else {
            panic!("not get: {command:?}");
        };
        assert_eq!(settings.message_id, 654_321);
    }

    #[test]
    fn parse_command__get_without_an_id__returns_error() {
        let result = parse_command(&["get"]);

        assert!(result.is_err());
    }

    #[test]
    fn parse_command__attachment_with_output__returns_ids_and_path() {
        let command = parse_command(&["attachment", "5", "9", "--output", "report.pdf"]).unwrap();

        let Command::Attachment(settings) = command else {
            panic!("not attachment: {command:?}");
        };
        assert_eq!(
            (settings.message_id, settings.attachment_id, settings.output),
            (5, 9, Some(PathBuf::from("report.pdf")))
        );
    }

    #[test]
    fn parse_command__attachment_without_output__has_no_path() {
        let command = parse_command(&["attachment", "5", "9"]).unwrap();

        let Command::Attachment(settings) = command else {
            panic!("not attachment: {command:?}");
        };
        assert_eq!(settings.output, None);
    }

    #[test]
    fn parse_command__watch_without_since__has_none() {
        let command = parse_command(&["watch"]).unwrap();

        let Command::Watch(settings) = command else {
            panic!("not watch: {command:?}");
        };
        assert_eq!(settings.since, None);
    }

    #[test]
    fn parse_command__watch_since_zero__replays_from_the_start() {
        let command = parse_command(&["watch", "--since", "0"]).unwrap();

        let Command::Watch(settings) = command else {
            panic!("not watch: {command:?}");
        };
        assert_eq!(settings.since, Some(0));
    }

    #[test]
    fn parse_command__watch_since_negative__returns_error() {
        let result = parse_command(&["watch", "--since", "-1"]);

        assert!(result.is_err());
    }

    #[test]
    fn parse_command__status_json__is_set() {
        let command = parse_command(&["status", "--json"]).unwrap();

        let Command::Status(settings) = command else {
            panic!("not status: {command:?}");
        };
        assert!(settings.json);
    }

    #[test]
    fn parse_command__failures_with_flags__returns_them() {
        let command = parse_command(&["failures", "--message-id", "5", "--limit", "20"]).unwrap();

        let Command::Failures(settings) = command else {
            panic!("not failures: {command:?}");
        };
        assert_eq!((settings.message_id, settings.limit), (Some(5), Some(20)));
    }
}
