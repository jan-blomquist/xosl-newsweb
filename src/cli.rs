//! The command line: its settings, and the `run` command.
//!
//! Each setting is a flag and an environment variable, defined once here;
//! `--help` lists them.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::builder::BoolishValueParser;
use clap::{Args, Parser, Subcommand};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::clock::{Clock, SystemClock};
use crate::exchange::client::{Client, Exchange, HttpTransport};
use crate::files::{Files, FilesError};
use crate::ingest::{self, JobSettings, Jobs};
use crate::logging::{self, InitError, LogFormat};
use crate::store::{Store, StoreError};

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
    #[error("cannot listen for SIGINT and SIGTERM")]
    Signal(#[source] std::io::Error),
}

/// The entry point of the binary.
pub async fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(settings) => run(&settings).await,
    };
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
        ingest = settings.ingest,
        request_interval_ms = settings.request_interval_ms,
        poll_interval_secs = settings.poll_interval_secs,
        lookback_days = settings.lookback_days,
        log_format = %settings.log_format,
        "starting"
    );
}

async fn run_logged(settings: &RunSettings) -> Result<(), RunError> {
    let layout = open_data_dir(&settings.data_dir)?;
    let store = Store::open(&layout.database).await?;
    let files = Files::open(layout.attachments.as_str(), [])?;
    // Creating the client sends no request; with ingest off it is not used.
    let transport = HttpTransport::new().map_err(RunError::Http)?;
    let client = Client::new(transport, settings.request_interval());
    let jobs = Jobs::new(&store, &files, &client, &SystemClock);

    let mirror_settings = settings.mirror_settings();
    let shutdown = CancellationToken::new();
    let mirror = run_mirror(&jobs, &mirror_settings, &shutdown);
    tokio::pin!(mirror);
    tokio::select! {
        result = &mut mirror => return result.map_err(RunError::from),
        signal = shutdown_signal() => signal.map_err(RunError::Signal)?,
    }
    tracing::info!("stopping");
    shutdown.cancel();
    mirror.await?;
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
        let Command::Run(settings) = Cli::try_parse_from(args)?.command;
        Ok(settings)
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
            log_format: LogFormat::Text,
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
            log_format: LogFormat::Json,
        };
        assert_eq!(settings, expected);
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
}
