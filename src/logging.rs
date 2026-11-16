//! Logging: what each level means, and how an error is written to the log.
//!
//! | Level | Meaning |
//! |---|---|
//! | `error` | The mirror cannot do part of its work and needs a person: the database or the attachment folder cannot be written, start-up failed, a disclosure or an attachment will not be tried again. |
//! | `warn` | One attempt failed and will be made again: a listing, a read, a download, a retry of a request, a truncated list. |
//! | `info` | Normal progress: start-up with the settings, coverage, a pass that did work, the periodic status, shutdown. |
//! | `debug` | Detail of each request to the exchange. |

use std::error::Error;
use std::fmt;
use std::io::IsTerminal;

use tracing::Subscriber;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// How a log line is written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum LogFormat {
    /// Human-readable lines; coloured only when stderr is a terminal.
    #[default]
    Text,
    /// One JSON object per line.
    Json,
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Json => "json",
        })
    }
}

/// Logging could not be set up.
#[derive(Debug, thiserror::Error)]
#[error("cannot set up logging")]
pub struct InitError(#[source] tracing_subscriber::util::TryInitError);

/// Installs the global subscriber: log lines go to stderr, at the level
/// `RUST_LOG` sets; `info` by default. Text lines are coloured only when
/// stderr is a terminal.
pub fn init(format: LogFormat) -> Result<(), InitError> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let ansi = std::io::stderr().is_terminal();
    tracing_subscriber::registry()
        .with(layer(format, ansi, std::io::stderr))
        .with(filter)
        .try_init()
        .map_err(InitError)
}

/// The layer that writes events to `writer` in `format`. `ansi` applies to
/// the text format only; the JSON format has no colour.
pub fn layer<S, W>(format: LogFormat, ansi: bool, writer: W) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let layer = tracing_subscriber::fmt::layer().with_writer(writer);
    match format {
        LogFormat::Text => layer.with_ansi(ansi).boxed(),
        LogFormat::Json => layer.json().boxed(),
    }
}

/// An error with its causes, for a log field: `outer: cause: root cause`.
/// `Display` of an error gives its own message only, which often leaves out
/// what went wrong underneath.
pub fn chain(error: &dyn Error) -> impl fmt::Display + '_ {
    Chain(error)
}

struct Chain<'a>(&'a dyn Error);

impl fmt::Display for Chain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        let mut shown = self.0.to_string();
        let mut cause = self.0.source();
        while let Some(error) = cause {
            let message = error.to_string();
            // An error whose own message ends with its cause's, or that only
            // wraps its cause, would repeat the cause.
            if !shown.ends_with(&message) {
                write!(f, ": {message}")?;
                shown = message;
            }
            cause = error.source();
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::io;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    /// A writer into memory, for reading back what was logged.
    #[derive(Clone, Default)]
    pub(crate) struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Capture {
        pub(crate) fn contents(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    impl io::Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Capture {
        type Writer = Capture;

        fn make_writer(&'a self) -> Capture {
            self.clone()
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use clap::Parser;
    use tracing_subscriber::Registry;

    use super::testing::Capture;
    use super::*;

    /// Logs one `info` event with a field through `layer(format, ansi, ..)`.
    fn log_one_event(format: LogFormat, ansi: bool) -> String {
        let capture = Capture::default();
        let subscriber = Registry::default().with(layer(format, ansi, capture.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(rows = 3, "listed the day");
        });
        capture.contents()
    }

    #[derive(Debug, Parser)]
    struct FormatArg {
        #[arg(long, value_enum, default_value_t)]
        log_format: LogFormat,
    }

    fn parse_format(args: &[&str]) -> Result<LogFormat, clap::Error> {
        let args = ["test"].iter().chain(args);
        Ok(FormatArg::try_parse_from(args)?.log_format)
    }

    #[test]
    fn layer__text_without_colour__has_no_escape_sequence() {
        let output = log_one_event(LogFormat::Text, false);

        assert!(!output.contains('\u{1b}'), "output: {output:?}");
    }

    #[test]
    fn layer__text_with_colour__has_an_escape_sequence() {
        let output = log_one_event(LogFormat::Text, true);

        assert!(output.contains('\u{1b}'), "output: {output:?}");
    }

    #[test]
    fn layer__json__writes_a_json_line_with_level_message_and_field() {
        let output = log_one_event(LogFormat::Json, true);

        let line: serde_json::Value = serde_json::from_str(output.trim_end()).unwrap();
        let actual = (
            line["level"].as_str(),
            line["fields"]["message"].as_str(),
            line["fields"]["rows"].as_i64(),
        );
        assert_eq!(actual, (Some("INFO"), Some("listed the day"), Some(3)));
    }

    #[test]
    fn parse_format__no_flag__is_text() {
        let format = parse_format(&[]).unwrap();

        assert_eq!(format, LogFormat::Text);
    }

    #[test]
    fn parse_format__json__is_json() {
        let format = parse_format(&["--log-format", "json"]).unwrap();

        assert_eq!(format, LogFormat::Json);
    }

    #[test]
    fn parse_format__unknown_value__returns_error_naming_the_setting() {
        let message = parse_format(&["--log-format", "xml"])
            .unwrap_err()
            .to_string();

        assert!(message.contains("--log-format"), "message: {message}");
    }

    #[derive(Debug, thiserror::Error)]
    #[error("cannot open the store")]
    struct Outer(#[source] Middle);

    #[derive(Debug, thiserror::Error)]
    #[error("cannot read the file")]
    struct Middle(#[source] std::io::Error);

    #[derive(Debug, thiserror::Error)]
    #[error(transparent)]
    struct Wrapper(#[from] Middle);

    fn permission_denied() -> std::io::Error {
        std::io::Error::other("permission denied")
    }

    #[test]
    fn chain__error_without_a_cause__writes_its_message() {
        let text = chain(&permission_denied()).to_string();

        assert_eq!(text, "permission denied");
    }

    #[test]
    fn chain__error_with_two_causes__writes_all_three_messages_in_order() {
        let error = Outer(Middle(permission_denied()));

        let text = chain(&error).to_string();

        assert_eq!(
            text,
            "cannot open the store: cannot read the file: permission denied"
        );
    }

    #[derive(Debug, thiserror::Error)]
    #[error("https://api.example/list: {0}")]
    struct NamesItsCause(#[source] Middle);

    #[test]
    fn chain__message_that_ends_with_its_cause__writes_the_cause_once() {
        let error = NamesItsCause(Middle(permission_denied()));

        let text = chain(&error).to_string();

        assert_eq!(
            text,
            "https://api.example/list: cannot read the file: permission denied"
        );
    }

    #[test]
    fn chain__transparent_wrapper__writes_the_wrapped_message_once() {
        let error = Wrapper(Middle(permission_denied()));

        let text = chain(&error).to_string();

        assert_eq!(text, "cannot read the file: permission denied");
    }
}
