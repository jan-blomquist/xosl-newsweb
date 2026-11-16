//! The client commands against the real service on the loopback address,
//! with a store and an object store in memory.

#![allow(non_snake_case)]

use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::service::Routes;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use xosl_newsweb::client::{self, ClientError, Format, ListQuery};
use xosl_newsweb::clock::Clock;
use xosl_newsweb::exchange::{self, Fetched};
use xosl_newsweb::files::Files;
use xosl_newsweb::ingest::JobStates;
use xosl_newsweb::pb::newsweb_service_server::NewswebServiceServer;
use xosl_newsweb::server::{MirrorService, StatusSettings};
use xosl_newsweb::store::Store;

/// 2026-09-22 12:01 UTC.
const NOW_NS: i64 = 1_790_078_460_000_000_000;
/// 2026-09-22 12:00 UTC, in seconds.
const NOON_S: i64 = 1_790_078_400;
/// Long enough for any of these calls; it only keeps a broken test from
/// hanging.
const TIMEOUT: Duration = Duration::from_secs(30);

struct FixedClock(i64);

impl Clock for FixedClock {
    fn now_ns(&self) -> i64 {
        self.0
    }
}

/// The service, serving on a free port until the test ends.
struct Mirror {
    address: String,
    store: Arc<Store>,
    files: Arc<Files>,
}

impl Mirror {
    async fn start() -> Self {
        let store = Arc::new(Store::open_in_memory().await.unwrap());
        let files = Arc::new(Files::open("memory:///", []).unwrap());
        let service = MirrorService::new(
            Arc::clone(&store),
            Arc::clone(&files),
            Arc::new(JobStates::default()),
            FixedClock(NOW_NS),
            StatusSettings {
                ingest_enabled: false,
                lookback_days: 3,
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let routes = Routes::new(NewswebServiceServer::new(service));
        tokio::spawn(
            Server::builder()
                .add_routes(routes)
                .serve_with_incoming(TcpIncoming::from(listener)),
        );
        Self {
            address,
            store,
            files,
        }
    }

    /// Stores each message read in full; each writes a `DISCOVERED` event.
    async fn store_messages(&self, messages: &[Value]) {
        for message in messages {
            self.store
                .store_message(&fetched(message, true), NOW_NS)
                .await
                .unwrap();
        }
    }
}

/// A message in the exchange's format, published `seconds` after noon on
/// 2026-09-22 UTC.
fn message(message_id: i64, sign: &str, seconds: i64) -> Value {
    let time = jiff::Timestamp::from_second(NOON_S + seconds).unwrap();
    json!({
        "messageId": message_id,
        "issuerId": 4242, "issuerName": "Example Shipping ASA", "issuerSign": sign,
        "publishedTime": time.to_string(),
        "title": format!("Report {message_id}"),
        "body": "Revenue rose.",
        "category": [{"id": 1002}],
        "attachments": [],
    })
}

fn with_attachment(mut message: Value, attachment_id: i64) -> Value {
    message["attachments"] = json!([{"id": attachment_id, "name": "report.pdf"}]);
    message
}

fn fetched(message: &Value, read_in_full: bool) -> Fetched {
    let document = message.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, read_in_full).unwrap(),
        document,
    }
}

/// An address that nothing listens on: a port that was bound and released.
async fn closed_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

async fn within_timeout<T>(call: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(TIMEOUT, call)
        .await
        .expect("the command ends within the timeout")
}

fn text(output: Vec<u8>) -> String {
    String::from_utf8(output).unwrap()
}

/// Cancels `stop` once `lines` lines were written.
struct StopAfterLines {
    written: Vec<u8>,
    lines: usize,
    stop: CancellationToken,
}

impl Write for StopAfterLines {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buffer);
        let seen = self.written.iter().filter(|byte| **byte == b'\n').count();
        if seen >= self.lines {
            self.stop.cancel();
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

async fn run_list(address: &str, query: &ListQuery, format: Format) -> Result<String, ClientError> {
    let mut output = Vec::new();
    within_timeout(client::list(address, query, format, &mut output)).await?;
    Ok(text(output))
}

async fn listed_ids(address: &str, query: &ListQuery) -> Vec<String> {
    let output = run_list(address, query, Format::Json).await.unwrap();
    output
        .lines()
        .map(|line| {
            let parsed: Value = serde_json::from_str(line).unwrap();
            parsed["messageId"].as_str().unwrap().to_owned()
        })
        .collect()
}

async fn mirror_with_three_disclosures() -> Mirror {
    let mirror = Mirror::start().await;
    mirror
        .store_messages(&[
            message(1, "EXSH", 10),
            message(2, "OTHR", 20),
            message(3, "EXSH", 30),
        ])
        .await;
    mirror
}

// list

#[tokio::test]
async fn list__stored_disclosures__prints_one_line_each_newest_first() {
    let mirror = mirror_with_three_disclosures().await;

    let output = run_list(&mirror.address, &ListQuery::default(), Format::Text)
        .await
        .unwrap();

    let titles: Vec<&str> = output
        .lines()
        .map(|line| line.rsplit(' ').next().unwrap())
        .collect();
    assert_eq!(titles, ["3", "2", "1"]);
}

#[tokio::test]
async fn list__text_line__has_time_id_sign_and_title() {
    let mirror = mirror_with_three_disclosures().await;
    let query = ListQuery {
        limit: Some(1),
        ..Default::default()
    };

    let output = run_list(&mirror.address, &query, Format::Text)
        .await
        .unwrap();

    assert_eq!(output, "2026-09-22T12:00:30Z 3 EXSH Report 3\n");
}

#[tokio::test]
async fn list__sign_given__prints_only_that_issuer() {
    let mirror = mirror_with_three_disclosures().await;
    let query = ListQuery {
        signs: vec!["OTHR".into()],
        ..Default::default()
    };

    let ids = listed_ids(&mirror.address, &query).await;

    assert_eq!(ids, ["2"]);
}

#[tokio::test]
async fn list__limit_below_the_matches__stops_after_the_limit_with_the_newest() {
    let mirror = mirror_with_three_disclosures().await;
    let query = ListQuery {
        limit: Some(2),
        ..Default::default()
    };

    let ids = listed_ids(&mirror.address, &query).await;

    assert_eq!(ids, ["3", "2"]);
}

#[tokio::test]
async fn list__from_and_before__from_is_inclusive_and_before_is_exclusive() {
    let mirror = mirror_with_three_disclosures().await;
    let query = ListQuery {
        from_ns: (NOON_S + 20) * 1_000_000_000,
        before_ns: (NOON_S + 30) * 1_000_000_000,
        ..Default::default()
    };

    let ids = listed_ids(&mirror.address, &query).await;

    assert_eq!(ids, ["2"]);
}

#[tokio::test]
async fn list__json__prints_one_json_object_per_line_with_the_message_id() {
    let mirror = mirror_with_three_disclosures().await;

    let ids = listed_ids(&mirror.address, &ListQuery::default()).await;

    assert_eq!(ids, ["3", "2", "1"]);
}

// get

#[tokio::test]
async fn get__stored_disclosure__prints_its_fields_and_body() {
    let mirror = Mirror::start().await;
    mirror
        .store_messages(&[with_attachment(message(7, "EXSH", 0), 9)])
        .await;
    let mut output = Vec::new();

    within_timeout(client::get(&mirror.address, 7, Format::Text, &mut output))
        .await
        .unwrap();

    let view = text(output);
    let expected_parts = [
        "message id: 7\n",
        "issuer: Example Shipping ASA (EXSH)\n",
        "title: Report 7\n",
        "  9 pending - - report.pdf\n",
        "\n\nRevenue rose.\n",
    ];
    let missing: Vec<&str> = expected_parts
        .into_iter()
        .filter(|part| !view.contains(part))
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "view: {view}");
}

#[tokio::test]
async fn get__unknown_id__returns_not_found_with_the_servers_message() {
    let mirror = Mirror::start().await;
    let mut output = Vec::new();

    let result = within_timeout(client::get(&mirror.address, 12, Format::Text, &mut output)).await;

    let message = result.unwrap_err().to_string();
    assert!(message.starts_with("not found: "), "message: {message}");
}

// attachment

async fn mirror_with_stored_attachment(bytes: &[u8]) -> Mirror {
    let mirror = Mirror::start().await;
    mirror
        .store_messages(&[with_attachment(message(1, "EXSH", 0), 9)])
        .await;
    let file = mirror
        .files
        .put(1, 9, Bytes::copy_from_slice(bytes))
        .await
        .unwrap();
    mirror
        .store
        .mark_attachment_stored(1, 9, &file, NOW_NS)
        .await
        .unwrap();
    mirror
}

#[tokio::test]
async fn attachment__to_a_writer__writes_exactly_the_stored_bytes() {
    let stored: Vec<u8> = (0..300_000_u32).map(|i| (i % 251) as u8).collect();
    let mirror = mirror_with_stored_attachment(&stored).await;
    let mut output = Vec::new();

    within_timeout(client::attachment(&mirror.address, 1, 9, || {
        Ok(&mut output)
    }))
    .await
    .unwrap();

    assert_eq!(output, stored);
}

#[tokio::test]
async fn attachment__to_a_file__writes_exactly_the_stored_bytes() {
    let stored = b"%PDF-1.4 invented content".to_vec();
    let mirror = mirror_with_stored_attachment(&stored).await;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("report.pdf");

    within_timeout(client::attachment(&mirror.address, 1, 9, || {
        std::fs::File::create(&path)
    }))
    .await
    .unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), stored);
}

#[tokio::test]
async fn attachment__pending__returns_failed_precondition_and_opens_no_output() {
    let mirror = Mirror::start().await;
    mirror
        .store_messages(&[with_attachment(message(1, "EXSH", 0), 9)])
        .await;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("report.pdf");

    let result = within_timeout(client::attachment(&mirror.address, 1, 9, || {
        std::fs::File::create(&path)
    }))
    .await;

    let outcome = (result.unwrap_err().to_string(), path.exists());
    assert!(
        outcome.0.starts_with("failed precondition: ") && !outcome.1,
        "outcome: {outcome:?}"
    );
}

// watch

#[tokio::test]
async fn watch__since_zero__prints_the_stored_events_in_order() {
    let mirror = mirror_with_three_disclosures().await;
    let stop = CancellationToken::new();
    let mut output = StopAfterLines {
        written: Vec::new(),
        lines: 3,
        stop: stop.clone(),
    };

    within_timeout(client::watch(
        &mirror.address,
        Some(0),
        Format::Text,
        &mut output,
        &stop,
    ))
    .await
    .unwrap();

    let lines: Vec<String> = text(output.written).lines().map(str::to_owned).collect();
    let seqs_and_kinds: Vec<(&str, &str)> = lines
        .iter()
        .map(|line| {
            let mut words = line.split(' ');
            (words.next().unwrap(), words.nth(1).unwrap())
        })
        .collect();
    assert_eq!(
        seqs_and_kinds,
        [
            ("1", "discovered"),
            ("2", "discovered"),
            ("3", "discovered")
        ]
    );
}

#[tokio::test]
async fn watch__since_a_seq__prints_only_later_events() {
    let mirror = mirror_with_three_disclosures().await;
    let stop = CancellationToken::new();
    let mut output = StopAfterLines {
        written: Vec::new(),
        lines: 1,
        stop: stop.clone(),
    };

    within_timeout(client::watch(
        &mirror.address,
        Some(2),
        Format::Json,
        &mut output,
        &stop,
    ))
    .await
    .unwrap();

    let event: Value = serde_json::from_str(text(output.written).trim_end()).unwrap();
    assert_eq!(event["seq"], "3");
}

// status

#[tokio::test]
async fn status__stored_disclosures__reports_the_counts_and_latest_seq() {
    let mirror = mirror_with_three_disclosures().await;
    let mut output = Vec::new();

    within_timeout(client::status(&mirror.address, Format::Text, &mut output))
        .await
        .unwrap();

    let view = text(output);
    let expected_parts = [
        "ingest: off\n",
        "disclosures: 3 (0 unread, 0 read failed)\n",
        "latest event: 3\n",
    ];
    let missing: Vec<&str> = expected_parts
        .into_iter()
        .filter(|part| !view.contains(part))
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "view: {view}");
}

// failures

#[tokio::test]
async fn failures__recorded_list_failure__prints_it() {
    let mirror = Mirror::start().await;
    let day = jiff::civil::date(2026, 9, 21);
    mirror
        .store
        .record_list_failure(day, "timeout", NOW_NS)
        .await
        .unwrap();
    let mut output = Vec::new();

    within_timeout(client::failures(
        &mirror.address,
        0,
        None,
        Format::Text,
        &mut output,
    ))
    .await
    .unwrap();

    assert_eq!(
        text(output),
        "2026-09-22T12:01:00Z list transient day 2026-09-21 timeout\n"
    );
}

// unreachable

#[tokio::test]
async fn commands__nothing_listens_on_the_address__report_unreachable() {
    let address = closed_address().await;
    let mut output = Vec::new();
    let stop = CancellationToken::new();

    let results = [
        within_timeout(client::list(
            &address,
            &ListQuery::default(),
            Format::Text,
            &mut output,
        ))
        .await,
        within_timeout(client::get(&address, 1, Format::Text, &mut output)).await,
        within_timeout(client::attachment(&address, 1, 9, || Ok(io::sink()))).await,
        within_timeout(client::watch(
            &address,
            Some(0),
            Format::Text,
            &mut output,
            &stop,
        ))
        .await,
        within_timeout(client::status(&address, Format::Text, &mut output)).await,
        within_timeout(client::failures(
            &address,
            0,
            None,
            Format::Text,
            &mut output,
        ))
        .await,
    ];

    let messages: Vec<String> = results
        .into_iter()
        .map(|result| result.unwrap_err().to_string())
        .collect();
    let not_unreachable: Vec<&String> = messages
        .iter()
        .filter(|message| !message.starts_with("unreachable: "))
        .collect();
    assert!(not_unreachable.is_empty(), "messages: {messages:?}");
}
