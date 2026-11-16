//! The command-line client: the commands that call a running mirror over
//! gRPC and print what it answers.
//!
//! The formatting and parsing functions are pure. The command functions
//! (`list`, `get`, `attachment`, `watch`, `status`, `failures`) make one call
//! and write to the `Write` they are given.

use std::io::{self, Write};

use jiff::Timestamp;
use jiff::civil::Date;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tonic::Code;
use tonic::transport::{Channel, Endpoint};

use crate::logging;
use crate::pb::newsweb_service_client::NewswebServiceClient;
use crate::pb::{self, attachment, event, failure};

/// The time zone whose calendar days a `<TIME>` date names.
const EXCHANGE_TIME_ZONE: &str = "Europe/Oslo";

/// How long a command waits for the connection to the mirror.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// How a data command prints what the mirror answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Readable lines.
    Text,
    /// One proto-JSON object per line.
    Json,
}

/// Why a command failed.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("unreachable: {cause}")]
    Unreachable { cause: String },
    #[error("{}: {}", status_words(.0.code()), .0.message())]
    Status(#[from] tonic::Status),
    #[error("cannot write the output")]
    Output(#[source] io::Error),
    #[error("cannot encode a message as JSON")]
    Json(#[source] serde_json::Error),
}

impl ClientError {
    /// The reader of the output closed it, for example `| head`: not a
    /// failure of the command.
    pub fn is_closed_output(&self) -> bool {
        matches!(self, Self::Output(error) if error.kind() == io::ErrorKind::BrokenPipe)
    }
}

/// Why a `<TIME>` was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TimeError {
    #[error(
        "{0:?} is neither an RFC 3339 timestamp (2026-11-16T08:00:00Z) nor a date (2026-11-16)"
    )]
    Format(String),
    #[error("{0:?} is outside the range of times")]
    OutOfRange(String),
}

// Pure functions: parsing and formatting.

/// Parses a `<TIME>` into nanoseconds since 1970-01-01 UTC: an RFC 3339
/// timestamp, or a date, which is the start of that day in Oslo.
pub fn parse_time(text: &str) -> Result<i64, TimeError> {
    let timestamp = match text.parse::<Timestamp>() {
        Ok(timestamp) => timestamp,
        Err(_) => {
            let format_error = || TimeError::Format(text.to_owned());
            let date: Date = text.parse().map_err(|_| format_error())?;
            let start = date.in_tz(EXCHANGE_TIME_ZONE).map_err(|_| format_error())?;
            start.timestamp()
        }
    };
    i64::try_from(timestamp.as_nanosecond()).map_err(|_| TimeError::OutOfRange(text.to_owned()))
}

/// A time as RFC 3339 in UTC, to the second.
pub fn format_time(at_ns: i64) -> String {
    Timestamp::from_nanosecond(i128::from(at_ns)).map_or_else(
        |_| format!("{at_ns} ns"),
        |timestamp| timestamp.strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
    )
}

/// `format_time`, or `never` for a time of 0, which the contract uses for
/// something that has not happened.
pub fn format_time_or_never(at_ns: i64) -> String {
    if at_ns == 0 {
        "never".to_owned()
    } else {
        format_time(at_ns)
    }
}

/// A gRPC status code in plain words.
pub fn status_words(code: Code) -> &'static str {
    match code {
        Code::Ok => "ok",
        Code::Cancelled => "cancelled",
        Code::Unknown => "unknown error",
        Code::InvalidArgument => "invalid argument",
        Code::DeadlineExceeded => "deadline exceeded",
        Code::NotFound => "not found",
        Code::AlreadyExists => "already exists",
        Code::PermissionDenied => "permission denied",
        Code::ResourceExhausted => "resource exhausted",
        Code::FailedPrecondition => "failed precondition",
        Code::Aborted => "aborted",
        Code::OutOfRange => "out of range",
        Code::Unimplemented => "unimplemented",
        Code::Internal => "internal error",
        Code::Unavailable => "unavailable",
        Code::DataLoss => "data loss",
        Code::Unauthenticated => "unauthenticated",
    }
}

/// The lower-case name of an event kind.
pub fn kind_words(kind: event::Kind) -> &'static str {
    match kind {
        event::Kind::Unspecified => "unknown",
        event::Kind::Discovered => "discovered",
        event::Kind::Revised => "revised",
        event::Kind::Corrected => "corrected",
        event::Kind::AttachmentStored => "attachment-stored",
        event::Kind::AttachmentFailed => "attachment-failed",
    }
}

/// The name of a failure stage.
pub fn stage_words(stage: failure::Stage) -> &'static str {
    match stage {
        failure::Stage::Unspecified => "unknown",
        failure::Stage::List => "list",
        failure::Stage::Read => "read",
        failure::Stage::Attachment => "attachment",
    }
}

fn state_words(state: attachment::State) -> &'static str {
    match state {
        attachment::State::Unspecified => "unknown",
        attachment::State::Pending => "pending",
        attachment::State::Stored => "stored",
        attachment::State::Failed => "failed",
    }
}

fn issuer_sign(disclosure: &pb::Disclosure) -> &str {
    disclosure.issuer.as_ref().map_or("", |issuer| &issuer.sign)
}

/// One line of `list`: publication time, message id, issuer sign, title;
/// marked when a later disclosure corrects it.
pub fn disclosure_line(disclosure: &pb::Disclosure) -> String {
    let mut line = format!(
        "{} {} {} {}",
        format_time(disclosure.published_at_ns),
        disclosure.message_id,
        issuer_sign(disclosure),
        disclosure.title
    );
    if disclosure.corrected_by_message_id != 0 {
        line.push_str(&format!(
            " (corrected by {})",
            disclosure.corrected_by_message_id
        ));
    }
    line
}

/// The view of `get`: labelled fields, the attachments, a blank line and
/// the body. Ends with a newline.
pub fn disclosure_view(disclosure: &pb::Disclosure) -> String {
    let mut lines = vec![
        format!("message id: {}", disclosure.message_id),
        format!("published: {}", format_time(disclosure.published_at_ns)),
        format!("received: {}", format_time(disclosure.received_at_ns)),
    ];
    if let Some(issuer) = &disclosure.issuer {
        lines.push(format!("issuer: {} ({})", issuer.name, issuer.sign));
    }
    if let Some(instrument) = &disclosure.instrument {
        lines.push(format!("instrument: {}", instrument.name));
    }
    if !disclosure.markets.is_empty() {
        lines.push(format!("markets: {}", disclosure.markets.join(", ")));
    }
    if !disclosure.categories.is_empty() {
        let categories: Vec<String> = disclosure
            .categories
            .iter()
            .map(|category| format!("{} {}", category.id, category.name_en))
            .collect();
        lines.push(format!("categories: {}", categories.join(", ")));
    }
    lines.push(format!("title: {}", disclosure.title));
    let flags: Vec<&str> = [
        (disclosure.test, "test"),
        (!disclosure.read_in_full, "not read in full yet"),
        (disclosure.read_failed, "read failed"),
    ]
    .into_iter()
    .filter_map(|(set, name)| set.then_some(name))
    .collect();
    if !flags.is_empty() {
        lines.push(format!("flags: {}", flags.join(", ")));
    }
    if disclosure.correction_for_message_id != 0 {
        lines.push(format!(
            "corrects: {}",
            disclosure.correction_for_message_id
        ));
    }
    if disclosure.corrected_by_message_id != 0 {
        lines.push(format!(
            "corrected by: {}",
            disclosure.corrected_by_message_id
        ));
    }
    if !disclosure.attachments.is_empty() {
        lines.push("attachments:".to_owned());
        lines.extend(disclosure.attachments.iter().map(attachment_line));
    }
    lines.push(String::new());
    lines.push(disclosure.body.clone());
    lines.join("\n") + "\n"
}

fn attachment_line(attachment: &pb::Attachment) -> String {
    let state =
        attachment::State::try_from(attachment.state).unwrap_or(attachment::State::Unspecified);
    let size = if attachment.size_bytes > 0 {
        format!("{} bytes", attachment.size_bytes)
    } else {
        "-".to_owned()
    };
    let content_type = if attachment.content_type.is_empty() {
        "-"
    } else {
        &attachment.content_type
    };
    format!(
        "  {} {} {} {} {}",
        attachment.id,
        state_words(state),
        size,
        content_type,
        attachment.name
    )
}

/// One line of `watch`: seq, time, kind, message id, issuer sign, title, and
/// the attachment id for the attachment kinds.
pub fn event_line(event: &pb::Event) -> String {
    let kind = event::Kind::try_from(event.kind).unwrap_or(event::Kind::Unspecified);
    let empty = pb::Disclosure::default();
    let disclosure = event.disclosure.as_ref().unwrap_or(&empty);
    let mut line = format!(
        "{} {} {} {} {} {}",
        event.seq,
        format_time(event.at_ns),
        kind_words(kind),
        disclosure.message_id,
        issuer_sign(disclosure),
        disclosure.title
    );
    if matches!(
        kind,
        event::Kind::AttachmentStored | event::Kind::AttachmentFailed
    ) {
        line.push_str(&format!(" attachment {}", event.attachment_id));
    }
    line
}

/// One line of `failures`: time, stage, permanent or transient, what it was
/// about, and the detail.
pub fn failure_line(failure: &pb::Failure) -> String {
    let stage = failure::Stage::try_from(failure.stage).unwrap_or(failure::Stage::Unspecified);
    let about = match stage {
        failure::Stage::List => format!("day {}", failure.day),
        failure::Stage::Attachment => format!(
            "message {} attachment {}",
            failure.message_id, failure.attachment_id
        ),
        failure::Stage::Read | failure::Stage::Unspecified => {
            format!("message {}", failure.message_id)
        }
    };
    format!(
        "{} {} {} {} {}",
        format_time(failure.at_ns),
        stage_words(stage),
        if failure.permanent {
            "permanent"
        } else {
            "transient"
        },
        about,
        failure.detail
    )
}

fn day_ranges(ranges: &[pb::DayRange]) -> String {
    if ranges.is_empty() {
        return "none".to_owned();
    }
    let words: Vec<String> = ranges
        .iter()
        .map(|range| {
            if range.first == range.last {
                range.first.clone()
            } else {
                format!("{}..{}", range.first, range.last)
            }
        })
        .collect();
    words.join(", ")
}

fn job_line(job: &pb::JobStatus) -> String {
    let mut line = format!(
        "  {}: last success {}",
        job.name,
        format_time_or_never(job.last_success_at_ns)
    );
    if job.last_failure_at_ns != 0 || !job.last_failure.is_empty() {
        line.push_str(&format!(
            "; last failure {}: {}",
            format_time_or_never(job.last_failure_at_ns),
            job.last_failure
        ));
    }
    line
}

/// The view of `status`. Ends with a newline.
pub fn status_view(status: &pb::Status) -> String {
    let counts = status.counts.unwrap_or_default();
    let coverage = status.coverage.clone().unwrap_or_default();
    let mut lines = vec![
        format!("version: {}", status.version),
        format!(
            "ingest: {}",
            if status.ingest_enabled { "on" } else { "off" }
        ),
        format!(
            "disclosures: {} ({} unread, {} read failed)",
            counts.disclosures, counts.disclosures_unread, counts.disclosures_failed
        ),
        format!(
            "attachments: {} pending, {} stored, {} failed",
            counts.attachments_pending, counts.attachments_stored, counts.attachments_failed
        ),
        format!("latest event: {}", status.latest_seq),
        format!("coverage from: {}", coverage.first_day),
        format!("sealed days: {}", day_ranges(&coverage.sealed)),
        format!("gaps: {}", day_ranges(&coverage.gaps)),
        format!("gap days: {}", coverage.gap_days),
    ];
    if !status.jobs.is_empty() {
        lines.push("jobs:".to_owned());
        lines.extend(status.jobs.iter().map(job_line));
    }
    lines.join("\n") + "\n"
}

fn json_line<T: Serialize>(message: &T) -> Result<String, ClientError> {
    serde_json::to_string(message).map_err(ClientError::Json)
}

// Commands: one call each.

/// What `list` asks for. A time of 0 is unbounded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub from_ns: i64,
    pub before_ns: i64,
    pub signs: Vec<String>,
    /// Stop after this many disclosures.
    pub limit: Option<u64>,
}

async fn connect(address: &str) -> Result<NewswebServiceClient<Channel>, ClientError> {
    let unreachable = |error: tonic::transport::Error| ClientError::Unreachable {
        cause: logging::chain(&error).to_string(),
    };
    let channel = Endpoint::from_shared(address.to_owned())
        .map_err(unreachable)?
        .connect_timeout(CONNECT_TIMEOUT)
        .connect()
        .await
        .map_err(unreachable)?;
    Ok(NewswebServiceClient::new(channel))
}

fn print(out: &mut impl Write, text: &str) -> Result<(), ClientError> {
    out.write_all(text.as_bytes()).map_err(ClientError::Output)
}

fn print_line(out: &mut impl Write, line: &str) -> Result<(), ClientError> {
    print(out, line)?;
    print(out, "\n")
}

/// `list`: prints the disclosures the query matches, newest first, and
/// cancels the call after `query.limit`.
pub async fn list(
    address: &str,
    query: &ListQuery,
    format: Format,
    out: &mut impl Write,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let request = pb::ListDisclosuresRequest {
        filter: Some(pb::DisclosureFilter {
            published_from_ns: query.from_ns,
            published_before_ns: query.before_ns,
            issuer_signs: query.signs.clone(),
        }),
    };
    let mut stream = client.list_disclosures(request).await?.into_inner();
    let mut printed = 0_u64;
    while query.limit.is_none_or(|limit| printed < limit) {
        let Some(response) = stream.message().await? else {
            break;
        };
        let Some(disclosure) = response.disclosure else {
            continue;
        };
        match format {
            Format::Text => print_line(out, &disclosure_line(&disclosure))?,
            Format::Json => print_line(out, &json_line(&disclosure)?)?,
        }
        printed += 1;
    }
    Ok(())
}

/// `get`: prints one disclosure.
pub async fn get(
    address: &str,
    message_id: i64,
    format: Format,
    out: &mut impl Write,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let response = client
        .get_disclosure(pb::GetDisclosureRequest { message_id })
        .await?
        .into_inner();
    let disclosure = response.disclosure.unwrap_or_default();
    match format {
        Format::Text => print(out, &disclosure_view(&disclosure)),
        Format::Json => print_line(out, &json_line(&disclosure)?),
    }
}

/// `attachment`: writes an attachment's bytes to the writer `open` returns,
/// chunk by chunk as they arrive. `open` is called once the mirror has
/// accepted the call, so a refused call creates no file.
pub async fn attachment<W: Write>(
    address: &str,
    message_id: i64,
    attachment_id: i64,
    open: impl FnOnce() -> io::Result<W>,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let request = pb::ReadAttachmentRequest {
        message_id,
        attachment_id,
    };
    let mut stream = client.read_attachment(request).await?.into_inner();
    let mut out = open().map_err(ClientError::Output)?;
    while let Some(response) = stream.message().await? {
        out.write_all(&response.chunk)
            .map_err(ClientError::Output)?;
    }
    out.flush().map_err(ClientError::Output)
}

/// `watch`: prints events until `stop` is cancelled or the server ends the
/// stream; both are success. Without `since_seq`, starts after the latest
/// event.
pub async fn watch(
    address: &str,
    since_seq: Option<i64>,
    format: Format,
    out: &mut impl Write,
    stop: &CancellationToken,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let since_seq = match since_seq {
        Some(seq) => seq,
        None => {
            let response = client.get_status(pb::GetStatusRequest {}).await?;
            response
                .into_inner()
                .status
                .map_or(0, |status| status.latest_seq)
        }
    };
    let mut stream = client
        .watch_events(pb::WatchEventsRequest { since_seq })
        .await?
        .into_inner();
    loop {
        let next = tokio::select! {
            () = stop.cancelled() => return Ok(()),
            next = stream.message() => next?,
        };
        let Some(response) = next else {
            return Ok(());
        };
        let Some(event) = response.event else {
            continue;
        };
        match format {
            Format::Text => print_line(out, &event_line(&event))?,
            Format::Json => print_line(out, &json_line(&event)?)?,
        }
        out.flush().map_err(ClientError::Output)?;
    }
}

/// `status`: prints what the mirror holds and how its jobs are doing.
pub async fn status(
    address: &str,
    format: Format,
    out: &mut impl Write,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let response = client.get_status(pb::GetStatusRequest {}).await?;
    let status = response.into_inner().status.unwrap_or_default();
    match format {
        Format::Text => print(out, &status_view(&status)),
        Format::Json => print_line(out, &json_line(&status)?),
    }
}

/// `failures`: prints the failure log, newest first, for one disclosure
/// when `message_id` is not 0, and cancels the call after `limit`.
pub async fn failures(
    address: &str,
    message_id: i64,
    limit: Option<u64>,
    format: Format,
    out: &mut impl Write,
) -> Result<(), ClientError> {
    let mut client = connect(address).await?;
    let mut stream = client
        .list_failures(pb::ListFailuresRequest { message_id })
        .await?
        .into_inner();
    let mut printed = 0_u64;
    while limit.is_none_or(|limit| printed < limit) {
        let Some(response) = stream.message().await? else {
            break;
        };
        let Some(failure) = response.failure else {
            continue;
        };
        match format {
            Format::Text => print_line(out, &failure_line(&failure))?,
            Format::Json => print_line(out, &json_line(&failure)?)?,
        }
        printed += 1;
    }
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 2026-11-16 08:00:00 UTC.
    const MORNING_NS: i64 = 1_794_816_000_000_000_000;

    fn disclosure() -> pb::Disclosure {
        pb::Disclosure {
            message_id: 654_321,
            issuer: Some(pb::Issuer {
                id: 4242,
                sign: "EXSH".into(),
                name: "Example Shipping ASA".into(),
            }),
            markets: vec!["XOSL".into(), "XOAX".into()],
            published_at_ns: MORNING_NS,
            received_at_ns: MORNING_NS + 5_000_000_000,
            title: "Second quarter 2026 results".into(),
            body: "Revenue rose.".into(),
            read_in_full: true,
            categories: vec![pb::Category {
                id: 1002,
                name_no: "HALVÅRSRAPPORT".into(),
                name_en: "HALF YEAR FINANCIAL REPORT".into(),
            }],
            ..Default::default()
        }
    }

    fn event(kind: event::Kind) -> pb::Event {
        pb::Event {
            seq: 17,
            kind: kind as i32,
            at_ns: MORNING_NS,
            disclosure: Some(disclosure()),
            attachment_id: 0,
        }
    }

    // parse_time

    #[test]
    fn parse_time__rfc3339_in_utc__returns_that_instant() {
        let parsed = parse_time("2026-11-16T08:00:00Z");

        assert_eq!(parsed, Ok(MORNING_NS));
    }

    #[test]
    fn parse_time__rfc3339_with_an_offset__returns_the_utc_instant() {
        let parsed = parse_time("2026-11-16T09:00:00+01:00");

        assert_eq!(parsed, Ok(MORNING_NS));
    }

    #[test]
    fn parse_time__date_in_winter__is_midnight_in_oslo_at_plus_one() {
        let parsed = parse_time("2026-11-16");

        let midnight_utc = parse_time("2026-11-15T23:00:00Z");
        assert_eq!(parsed, midnight_utc);
    }

    #[test]
    fn parse_time__date_in_summer__is_midnight_in_oslo_at_plus_two() {
        let parsed = parse_time("2026-07-01");

        let midnight_utc = parse_time("2026-06-30T22:00:00Z");
        assert_eq!(parsed, midnight_utc);
    }

    #[test]
    fn parse_time__dates_around_the_change_to_summer_time__differ_by_23_hours() {
        let day_before = parse_time("2026-03-28").unwrap();
        let change_day = parse_time("2026-03-29").unwrap();
        let day_after = parse_time("2026-03-30").unwrap();

        let hours_of_the_change_day = (day_after - change_day) / 3_600_000_000_000;
        let hours_before = (change_day - day_before) / 3_600_000_000_000;
        assert_eq!((hours_before, hours_of_the_change_day), (24, 23));
    }

    #[test]
    fn parse_time__dates_around_the_change_to_winter_time__differ_by_25_hours() {
        let change_day = parse_time("2026-10-25").unwrap();
        let day_after = parse_time("2026-10-26").unwrap();

        let hours = (day_after - change_day) / 3_600_000_000_000;
        assert_eq!(hours, 25);
    }

    #[test]
    fn parse_time__neither_timestamp_nor_date__returns_format_error_naming_the_text() {
        let error = parse_time("yesterday").unwrap_err();

        assert!(error.to_string().contains("\"yesterday\""), "{error}");
    }

    // format_time

    #[test]
    fn format_time__nanoseconds__is_rfc3339_in_utc_to_the_second() {
        let text = format_time(MORNING_NS + 999_000_000);

        assert_eq!(text, "2026-11-16T08:00:00Z");
    }

    #[test]
    fn format_time_or_never__zero__is_never() {
        let text = format_time_or_never(0);

        assert_eq!(text, "never");
    }

    #[test]
    fn format_time_or_never__a_time__is_formatted() {
        let text = format_time_or_never(MORNING_NS);

        assert_eq!(text, "2026-11-16T08:00:00Z");
    }

    // Wording

    #[test]
    fn status_words__codes__are_plain_words() {
        let cases = [
            (Code::NotFound, "not found"),
            (Code::FailedPrecondition, "failed precondition"),
            (Code::OutOfRange, "out of range"),
            (Code::Unavailable, "unavailable"),
        ];
        for (code, words) in cases {
            assert_eq!(status_words(code), words);
        }
    }

    #[test]
    fn client_error__status__shows_the_code_and_the_servers_message() {
        let error = ClientError::from(tonic::Status::not_found("no disclosure 12"));

        assert_eq!(error.to_string(), "not found: no disclosure 12");
    }

    #[test]
    fn client_error__unreachable__shows_the_cause() {
        let error = ClientError::Unreachable {
            cause: "connection refused".into(),
        };

        assert_eq!(error.to_string(), "unreachable: connection refused");
    }

    #[test]
    fn kind_words__every_kind__is_lower_case_with_hyphens() {
        let cases = [
            (event::Kind::Discovered, "discovered"),
            (event::Kind::Revised, "revised"),
            (event::Kind::Corrected, "corrected"),
            (event::Kind::AttachmentStored, "attachment-stored"),
            (event::Kind::AttachmentFailed, "attachment-failed"),
        ];
        for (kind, words) in cases {
            assert_eq!(kind_words(kind), words);
        }
    }

    #[test]
    fn stage_words__every_stage__is_its_name() {
        let cases = [
            (failure::Stage::List, "list"),
            (failure::Stage::Read, "read"),
            (failure::Stage::Attachment, "attachment"),
        ];
        for (stage, words) in cases {
            assert_eq!(stage_words(stage), words);
        }
    }

    // Text formats

    #[test]
    fn disclosure_line__a_disclosure__has_time_id_sign_and_title_in_order() {
        let line = disclosure_line(&disclosure());

        assert_eq!(
            line,
            "2026-11-16T08:00:00Z 654321 EXSH Second quarter 2026 results"
        );
    }

    #[test]
    fn disclosure_line__corrected__is_marked_with_the_correcting_id() {
        let corrected = pb::Disclosure {
            corrected_by_message_id: 654_400,
            ..disclosure()
        };

        let line = disclosure_line(&corrected);

        assert!(line.ends_with("(corrected by 654400)"), "{line}");
    }

    #[test]
    fn disclosure_view__a_disclosure__has_labelled_fields_attachments_and_body_after_a_blank_line()
    {
        let full = pb::Disclosure {
            instrument: Some(pb::Instrument {
                id: 1,
                name: "EXSH".into(),
                full_name: "Example Shipping ordinary share".into(),
            }),
            attachments: vec![pb::Attachment {
                id: 7,
                name: "report.pdf".into(),
                state: attachment::State::Stored as i32,
                content_type: "application/pdf".into(),
                size_bytes: 1234,
                sha256: String::new(),
            }],
            ..disclosure()
        };

        let view = disclosure_view(&full);

        let expected_parts = [
            "message id: 654321\n",
            "published: 2026-11-16T08:00:00Z\n",
            "received: 2026-11-16T08:00:05Z\n",
            "issuer: Example Shipping ASA (EXSH)\n",
            "instrument: EXSH\n",
            "markets: XOSL, XOAX\n",
            "categories: 1002 HALF YEAR FINANCIAL REPORT\n",
            "title: Second quarter 2026 results\n",
            "attachments:\n  7 stored 1234 bytes application/pdf report.pdf\n",
            "\n\nRevenue rose.\n",
        ];
        let missing: Vec<&str> = expected_parts
            .into_iter()
            .filter(|part| !view.contains(part))
            .collect();
        assert_eq!(missing, Vec::<&str>::new(), "view: {view}");
    }

    #[test]
    fn disclosure_view__no_flags_no_corrections__has_no_flags_or_correction_lines() {
        let view = disclosure_view(&disclosure());

        let unwanted = ["flags:", "corrects:", "corrected by:", "instrument:"];
        let found: Vec<&str> = unwanted
            .into_iter()
            .filter(|label| view.contains(label))
            .collect();
        assert_eq!(found, Vec::<&str>::new(), "view: {view}");
    }

    #[test]
    fn disclosure_view__every_flag_and_correction__shows_them() {
        let flagged = pb::Disclosure {
            test: true,
            read_in_full: false,
            read_failed: true,
            correction_for_message_id: 11,
            corrected_by_message_id: 13,
            ..disclosure()
        };

        let view = disclosure_view(&flagged);

        let expected_parts = [
            "flags: test, not read in full yet, read failed\n",
            "corrects: 11\n",
            "corrected by: 13\n",
        ];
        let missing: Vec<&str> = expected_parts
            .into_iter()
            .filter(|part| !view.contains(part))
            .collect();
        assert_eq!(missing, Vec::<&str>::new(), "view: {view}");
    }

    #[test]
    fn event_line__discovered__has_seq_time_kind_id_sign_and_title() {
        let line = event_line(&event(event::Kind::Discovered));

        assert_eq!(
            line,
            "17 2026-11-16T08:00:00Z discovered 654321 EXSH Second quarter 2026 results"
        );
    }

    #[test]
    fn event_line__attachment_kind__ends_with_the_attachment_id() {
        let stored = pb::Event {
            attachment_id: 7,
            ..event(event::Kind::AttachmentStored)
        };

        let line = event_line(&stored);

        assert!(
            line.contains(" attachment-stored ") && line.ends_with(" attachment 7"),
            "{line}"
        );
    }

    #[test]
    fn failure_line__list_failure__shows_the_day_and_transient() {
        let failure = pb::Failure {
            id: 1,
            at_ns: MORNING_NS,
            stage: failure::Stage::List as i32,
            day: "2026-11-15".into(),
            detail: "timeout".into(),
            ..Default::default()
        };

        let line = failure_line(&failure);

        assert_eq!(
            line,
            "2026-11-16T08:00:00Z list transient day 2026-11-15 timeout"
        );
    }

    #[test]
    fn failure_line__permanent_attachment_failure__shows_both_ids() {
        let failure = pb::Failure {
            at_ns: MORNING_NS,
            stage: failure::Stage::Attachment as i32,
            permanent: true,
            message_id: 5,
            attachment_id: 9,
            detail: "404".into(),
            ..Default::default()
        };

        let line = failure_line(&failure);

        assert!(
            line.contains(" attachment permanent message 5 attachment 9 404"),
            "{line}"
        );
    }

    #[test]
    fn failure_line__read_failure__shows_the_message_id() {
        let failure = pb::Failure {
            at_ns: MORNING_NS,
            stage: failure::Stage::Read as i32,
            message_id: 5,
            detail: "bad".into(),
            ..Default::default()
        };

        let line = failure_line(&failure);

        assert!(line.contains(" read transient message 5 bad"), "{line}");
    }

    fn a_status() -> pb::Status {
        pb::Status {
            version: "0.1.0".into(),
            ingest_enabled: true,
            jobs: vec![
                pb::JobStatus {
                    name: "poller".into(),
                    last_success_at_ns: MORNING_NS,
                    ..Default::default()
                },
                pb::JobStatus {
                    name: "reader".into(),
                    last_failure_at_ns: MORNING_NS,
                    last_failure: "timeout".into(),
                    ..Default::default()
                },
            ],
            coverage: Some(pb::Coverage {
                first_day: "2026-11-09".into(),
                lookback_days: 7,
                sealed: vec![
                    pb::DayRange {
                        first: "2026-11-09".into(),
                        last: "2026-11-12".into(),
                    },
                    pb::DayRange {
                        first: "2026-11-14".into(),
                        last: "2026-11-14".into(),
                    },
                ],
                gaps: vec![pb::DayRange {
                    first: "2026-11-13".into(),
                    last: "2026-11-13".into(),
                }],
                gap_days: 1,
            }),
            counts: Some(pb::Counts {
                disclosures: 10,
                disclosures_unread: 2,
                disclosures_failed: 1,
                attachments_pending: 3,
                attachments_stored: 4,
                attachments_failed: 5,
            }),
            latest_seq: 42,
        }
    }

    #[test]
    fn status_view__a_status__shows_counts_coverage_and_latest_seq() {
        let view = status_view(&a_status());

        let expected_parts = [
            "version: 0.1.0\n",
            "ingest: on\n",
            "disclosures: 10 (2 unread, 1 read failed)\n",
            "attachments: 3 pending, 4 stored, 5 failed\n",
            "latest event: 42\n",
            "coverage from: 2026-11-09\n",
            "sealed days: 2026-11-09..2026-11-12, 2026-11-14\n",
            "gaps: 2026-11-13\n",
            "gap days: 1\n",
        ];
        let missing: Vec<&str> = expected_parts
            .into_iter()
            .filter(|part| !view.contains(part))
            .collect();
        assert_eq!(missing, Vec::<&str>::new(), "view: {view}");
    }

    #[test]
    fn status_view__job_without_failure__shows_its_success_and_no_failure() {
        let view = status_view(&a_status());

        assert!(
            view.contains("  poller: last success 2026-11-16T08:00:00Z\n"),
            "{view}"
        );
    }

    #[test]
    fn status_view__job_that_never_succeeded_but_failed__shows_never_and_the_failure() {
        let view = status_view(&a_status());

        assert!(
            view.contains(
                "  reader: last success never; last failure 2026-11-16T08:00:00Z: timeout\n"
            ),
            "{view}"
        );
    }

    #[test]
    fn status_view__ingest_off_and_no_ranges__says_off_and_none() {
        let idle = pb::Status {
            ingest_enabled: false,
            jobs: vec![],
            coverage: None,
            ..a_status()
        };

        let view = status_view(&idle);

        assert!(
            view.contains("ingest: off\n") && !view.contains("jobs:"),
            "{view}"
        );
    }

    #[test]
    fn json_line__a_disclosure__is_one_proto_json_object_with_the_message_id() {
        let line = json_line(&disclosure()).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["messageId"], "654321");
    }

    #[test]
    fn is_closed_output__broken_pipe__is_true() {
        let error = ClientError::Output(io::ErrorKind::BrokenPipe.into());

        assert!(error.is_closed_output());
    }
}
