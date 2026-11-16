//! Health: whether the mirror is doing its work, as the gRPC health service
//! and the `health` command report it.

use std::fmt;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::service::Routes;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tonic_health::ServingStatus;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus as WireStatus;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::server::HealthReporter;

use crate::ingest::JobStatesSnapshot;
use crate::logging;

/// How often `serve` evaluates the health rule.
pub const EVALUATION_INTERVAL: Duration = Duration::from_secs(10);

/// How long the `health` command waits for an answer.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// The service names whose status follows the health rule: the empty name,
/// which means the whole server, and the mirror's own service.
const SERVICE_NAMES: [&str; 2] = ["", crate::pb::newsweb_service_server::SERVICE_NAME];

/// The shortest time the poller may go without a successful listing.
const MIN_SILENCE: Duration = Duration::from_secs(10 * 60);

/// How many poll intervals the poller may go without a successful listing.
const SILENT_POLL_INTERVALS: u32 = 3;

/// Whether the mirror is doing its work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Unhealthy { reason: String },
}

impl Health {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// The mirror is healthy when ingest is off, or when the poller's last
/// successful listing is at most `max(10 minutes, 3 × poll_interval)` old.
/// Before the first listing, the age counts from `started_at_ns`. Failed
/// reads and downloads, backfill gaps and quiet days do not matter. Times
/// are nanoseconds since 1970-01-01 UTC.
pub fn health(
    states: &JobStatesSnapshot,
    ingest_enabled: bool,
    now_ns: i64,
    started_at_ns: i64,
    poll_interval: Duration,
) -> Health {
    if !ingest_enabled {
        return Health::Healthy;
    }
    let limit = MIN_SILENCE.max(poll_interval.saturating_mul(SILENT_POLL_INTERVALS));
    let last_listing_ns = states.poller.last_success_ns;
    let counted_from_ns = last_listing_ns.unwrap_or(started_at_ns);
    let age_ns = i128::from(now_ns) - i128::from(counted_from_ns);
    if age_ns <= limit.as_nanos().try_into().unwrap_or(i128::MAX) {
        return Health::Healthy;
    }
    let last_listing = match last_listing_ns {
        Some(ns) => format!("the last successful listing was at {}", format_instant(ns)),
        None => "there has been no successful listing since start-up".to_owned(),
    };
    let mut reason = format!(
        "the poller has not listed NewsWeb for more than {} seconds: {last_listing}",
        limit.as_secs()
    );
    if let Some(failure) = &states.poller.last_failure {
        reason.push_str(&format!("; the last failure was: {}", failure.detail));
    }
    Health::Unhealthy { reason }
}

/// An instant as an RFC 3339 UTC timestamp.
fn format_instant(at_ns: i64) -> String {
    jiff::Timestamp::from_nanosecond(i128::from(at_ns))
        .map_or_else(|_| format!("{at_ns} ns"), |timestamp| timestamp.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("cannot serve gRPC")]
    Transport(#[source] tonic::transport::Error),
}

/// Serves `services` and the gRPC health service on `listener` until
/// `shutdown` is cancelled. `evaluate` is called once before the first request and then
/// every `interval`; the status of each service name in `SERVICE_NAMES`
/// follows its result. A change of status is logged.
pub async fn serve<F>(
    listener: TcpListener,
    services: Routes,
    evaluate: F,
    interval: Duration,
    shutdown: &CancellationToken,
) -> Result<(), ServeError>
where
    F: Fn() -> Health,
{
    let (reporter, health_service) = tonic_health::server::health_reporter();
    let mut current = evaluate();
    set_statuses(&reporter, &current).await;

    // Ends the evaluation too when the server stops by itself.
    let stop = shutdown.child_token();
    let server = Server::builder()
        .add_routes(services)
        .add_service(health_service)
        .serve_with_incoming_shutdown(TcpIncoming::from(listener), stop.cancelled());
    let server = async {
        let result = server.await;
        stop.cancel();
        result
    };
    let watch = watch(&reporter, &evaluate, interval, &mut current, &stop);
    let (result, ()) = tokio::join!(server, watch);
    result.map_err(ServeError::Transport)
}

/// Re-evaluates every `interval` until `shutdown` is cancelled.
async fn watch<F: Fn() -> Health>(
    reporter: &HealthReporter,
    evaluate: &F,
    interval: Duration,
    current: &mut Health,
    shutdown: &CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await;
    loop {
        tokio::select! {
            () = shutdown.cancelled() => return,
            _ = ticker.tick() => {}
        }
        let next = evaluate();
        if next.is_healthy() != current.is_healthy() {
            log_change(&next);
        }
        set_statuses(reporter, &next).await;
        *current = next;
    }
}

fn log_change(health: &Health) {
    match health {
        Health::Healthy => tracing::info!("the mirror is healthy"),
        Health::Unhealthy { reason } => tracing::error!(%reason, "the mirror is unhealthy"),
    }
}

async fn set_statuses(reporter: &HealthReporter, health: &Health) {
    let status = if health.is_healthy() {
        ServingStatus::Serving
    } else {
        ServingStatus::NotServing
    };
    for name in SERVICE_NAMES {
        reporter.set_service_status(name, status).await;
    }
}

/// What the `health` command found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Healthy,
    Unhealthy,
    Unreachable { cause: String },
}

impl Outcome {
    /// The process exit code: 0 when healthy, 1 otherwise.
    pub fn exit_code(&self) -> u8 {
        u8::from(*self != Self::Healthy)
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => f.write_str("healthy"),
            Self::Unhealthy => f.write_str("unhealthy"),
            Self::Unreachable { cause } => write!(f, "unreachable: {cause}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum CheckError {
    #[error("cannot connect")]
    Connect(#[source] tonic::transport::Error),
    #[error("the health call failed")]
    Call(#[source] tonic::Status),
    #[error("no answer within {} seconds", .0.as_secs())]
    Timeout(Duration),
}

/// Asks the health service at `address` (for example
/// `http://127.0.0.1:50051`) for the status of the whole server.
pub async fn check(address: &str, timeout: Duration) -> Outcome {
    match tokio::time::timeout(timeout, ask(address)).await {
        Ok(Ok(WireStatus::Serving)) => Outcome::Healthy,
        Ok(Ok(_)) => Outcome::Unhealthy,
        Ok(Err(error)) => unreachable_because(&error),
        Err(_) => unreachable_because(&CheckError::Timeout(timeout)),
    }
}

fn unreachable_because(error: &CheckError) -> Outcome {
    Outcome::Unreachable {
        cause: logging::chain(error).to_string(),
    }
}

async fn ask(address: &str) -> Result<WireStatus, CheckError> {
    let channel = tonic::transport::Endpoint::from_shared(address.to_owned())
        .map_err(CheckError::Connect)?
        .connect()
        .await
        .map_err(CheckError::Connect)?;
    let response = HealthClient::new(channel)
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await
        .map_err(CheckError::Call)?;
    Ok(response.into_inner().status())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::ingest::{JobFailure, JobState};

    const MINUTE_NS: i64 = 60 * 1_000_000_000;
    const START_NS: i64 = 1_800_000_000 * 1_000_000_000;

    fn polled_at(last_success_ns: Option<i64>, failure: Option<&str>) -> JobStatesSnapshot {
        JobStatesSnapshot {
            poller: JobState {
                last_pass_ended_ns: None,
                last_success_ns,
                last_failure: failure.map(|detail| JobFailure {
                    at_ns: START_NS,
                    detail: detail.to_owned(),
                }),
            },
            ..JobStatesSnapshot::default()
        }
    }

    fn at_minutes(minutes: i64) -> i64 {
        START_NS + minutes * MINUTE_NS
    }

    fn evaluate(states: &JobStatesSnapshot, now_minutes: i64, poll_secs: u64) -> Health {
        health(
            states,
            true,
            at_minutes(now_minutes),
            START_NS,
            Duration::from_secs(poll_secs),
        )
    }

    fn reason(health: Health) -> String {
        match health {
            Health::Healthy => String::new(),
            Health::Unhealthy { reason } => reason,
        }
    }

    #[test]
    fn health__ingest_off_and_nothing_listed_for_a_day__is_healthy() {
        let states = polled_at(None, Some("timeout"));

        let result = health(
            &states,
            false,
            at_minutes(24 * 60),
            START_NS,
            Duration::from_secs(60),
        );

        assert_eq!(result, Health::Healthy);
    }

    #[test]
    fn health__listing_within_the_limit__is_healthy() {
        let states = polled_at(Some(at_minutes(0)), None);

        let result = evaluate(&states, 10, 60);

        assert_eq!(result, Health::Healthy);
    }

    #[test]
    fn health__listing_older_than_the_limit__is_unhealthy_with_its_time() {
        let states = polled_at(Some(at_minutes(0)), None);

        let result = evaluate(&states, 11, 60);

        let expected = "the poller has not listed NewsWeb for more than 600 seconds: \
                        the last successful listing was at 2027-01-15T08:00:00Z";
        assert_eq!(reason(result), expected);
    }

    #[test]
    fn health__no_listing_yet_within_the_limit_of_start__is_healthy() {
        let states = polled_at(None, None);

        let result = evaluate(&states, 10, 60);

        assert_eq!(result, Health::Healthy);
    }

    #[test]
    fn health__no_listing_yet_past_the_limit__is_unhealthy_saying_there_was_none() {
        let states = polled_at(None, None);

        let result = evaluate(&states, 11, 60);

        let expected = "the poller has not listed NewsWeb for more than 600 seconds: \
                        there has been no successful listing since start-up";
        assert_eq!(reason(result), expected);
    }

    #[test]
    fn health__poll_interval_of_one_hour__allows_three_hours_without_a_listing() {
        let states = polled_at(Some(at_minutes(0)), None);

        let cases = [(180, true), (181, false)];
        for (now_minutes, expected) in cases {
            let result = evaluate(&states, now_minutes, 3600);
            assert_eq!(result.is_healthy(), expected, "now: {now_minutes} minutes");
        }
    }

    #[test]
    fn health__unhealthy_after_a_failed_listing__reason_names_the_failure() {
        let states = polled_at(None, Some("connection refused"));

        let result = evaluate(&states, 30, 60);

        assert!(reason(result).ends_with("the last failure was: connection refused"));
    }

    #[test]
    fn health__listing_current_but_other_jobs_failing__is_healthy() {
        let mut states = polled_at(Some(at_minutes(0)), None);
        let failure = Some(JobFailure {
            at_ns: START_NS,
            detail: "disk full".to_owned(),
        });
        states.reader.last_failure = failure.clone();
        states.downloader.last_failure = failure;

        let result = evaluate(&states, 5, 60);

        assert_eq!(result, Health::Healthy);
    }

    #[test]
    fn outcome__each_variant__has_its_line_and_exit_code() {
        let cases = [
            (Outcome::Healthy, "healthy", 0),
            (Outcome::Unhealthy, "unhealthy", 1),
            (
                Outcome::Unreachable {
                    cause: "cannot connect".to_owned(),
                },
                "unreachable: cannot connect",
                1,
            ),
        ];
        for (outcome, line, code) in cases {
            assert_eq!(
                (outcome.to_string(), outcome.exit_code()),
                (line.to_owned(), code)
            );
        }
    }
}
