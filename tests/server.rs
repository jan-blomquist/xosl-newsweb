//! Integration tests of the service: each call against a store in memory and
//! an object store in memory, with a fixed clock.

#![allow(non_snake_case)]

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use jiff::civil::Date;
use serde_json::{Value, json};
use tokio_stream::{Stream, StreamExt};
use tonic::{Code, Request, Status};
use xosl_newsweb::clock::Clock;
use xosl_newsweb::exchange::{self, Fetched};
use xosl_newsweb::files::Files;
use xosl_newsweb::ingest::JobStates;
use xosl_newsweb::pb::{self, newsweb_service_server::NewswebService};
use xosl_newsweb::server::{CHUNK_SIZE, DISCLOSURE_PAGE_SIZE, MirrorService, StatusSettings};
use xosl_newsweb::store::{Failure, ListedDay, Store};

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

struct Fixture {
    store: Arc<Store>,
    files: Arc<Files>,
    service: MirrorService<FixedClock>,
}

async fn fixture_with(settings: StatusSettings) -> Fixture {
    let store = Arc::new(Store::open_in_memory().await.unwrap());
    let files = Arc::new(Files::open("memory:///", []).unwrap());
    let states = Arc::new(JobStates::default());
    let service = MirrorService::new(
        Arc::clone(&store),
        Arc::clone(&files),
        states,
        FixedClock(NOW_NS),
        settings,
    );
    Fixture {
        store,
        files,
        service,
    }
}

async fn fixture() -> Fixture {
    fixture_with(StatusSettings {
        ingest_enabled: false,
        lookback_days: 3,
    })
    .await
}

/// A message in the exchange's format, published at noon on 2026-09-22
/// UTC. Tests set the keys their scenario is about.
fn message(message_id: i64) -> Value {
    json!({
        "messageId": message_id,
        "issuerId": 4242, "issuerName": "Example Shipping ASA", "issuerSign": "EXSH",
        "publishedTime": "2026-09-22T12:00:00Z",
        "title": "Second quarter 2026 results",
        "body": "Revenue rose.",
        "category": [{"id": 1002}],
        "attachments": [],
    })
}

fn with(mut message: Value, key: &str, value: Value) -> Value {
    message[key] = value;
    message
}

fn with_attachments(message: Value, attachment_ids: &[i64]) -> Value {
    let attachments: Vec<Value> = attachment_ids
        .iter()
        .map(|id| json!({"id": id, "name": format!("{id}.pdf")}))
        .collect();
    with(message, "attachments", json!(attachments))
}

/// Published `seconds` after noon.
fn published_after_noon(message: Value, seconds: i64) -> Value {
    let time = jiff::Timestamp::from_second(NOON_S + seconds).unwrap();
    with(message, "publishedTime", json!(time.to_string()))
}

fn fetched(message: &Value, read_in_full: bool) -> Fetched {
    let document = message.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, read_in_full).unwrap(),
        document,
    }
}

/// Stores the messages read in full; each writes one `DISCOVERED` event.
async fn store_messages(store: &Store, message_ids: impl IntoIterator<Item = i64>) {
    for message_id in message_ids {
        let full = fetched(&message(message_id), true);
        store.store_message(&full, NOW_NS).await.unwrap();
    }
}

/// The first `count` items of a stream, or all of them when it ends first.
async fn take<T>(stream: impl Stream<Item = Result<T, Status>>, count: usize) -> Vec<T> {
    let items = tokio::time::timeout(TIMEOUT, stream.take(count).collect::<Vec<_>>())
        .await
        .expect("the stream sends within the timeout");
    items.into_iter().map(Result::unwrap).collect()
}

async fn all<T>(stream: impl Stream<Item = Result<T, Status>>) -> Vec<T> {
    take(stream, usize::MAX).await
}

fn code<T>(result: Result<T, Status>) -> Code {
    result.err().map_or(Code::Ok, |status| status.code())
}

fn seqs(responses: &[pb::WatchEventsResponse]) -> Vec<i64> {
    responses
        .iter()
        .map(|response| response.event.as_ref().unwrap().seq)
        .collect()
}

fn watch(since_seq: i64) -> Request<pb::WatchEventsRequest> {
    Request::new(pb::WatchEventsRequest { since_seq })
}

// ListDisclosures

#[tokio::test]
async fn list_disclosures__more_than_a_page__returns_every_match_newest_first_without_tests() {
    let fixture = fixture().await;
    let count = i64::from(DISCLOSURE_PAGE_SIZE) + 20;
    for message_id in 1..=count {
        let row = published_after_noon(message(message_id), message_id);
        fixture
            .store
            .store_list_entry(&fetched(&row, false), NOW_NS)
            .await
            .unwrap();
    }
    let test = with(
        published_after_noon(message(count + 1), 0),
        "test",
        json!(true),
    );
    fixture
        .store
        .store_list_entry(&fetched(&test, false), NOW_NS)
        .await
        .unwrap();

    let response = fixture
        .service
        .list_disclosures(Request::new(pb::ListDisclosuresRequest::default()))
        .await
        .unwrap();
    let listed = all(response.into_inner()).await;

    let ids: Vec<i64> = listed
        .iter()
        .map(|response| response.disclosure.as_ref().unwrap().message_id)
        .collect();
    assert_eq!(ids, (1..=count).rev().collect::<Vec<_>>());
}

// GetDisclosure

async fn get(fixture: &Fixture, message_id: i64) -> Result<pb::Disclosure, Status> {
    let request = Request::new(pb::GetDisclosureRequest { message_id });
    let response = fixture.service.get_disclosure(request).await?;
    Ok(response.into_inner().disclosure.unwrap())
}

#[tokio::test]
async fn get_disclosure__stored__returns_it() {
    let fixture = fixture().await;
    store_messages(&fixture.store, [7]).await;

    let disclosure = get(&fixture, 7).await.unwrap();

    assert_eq!(disclosure.title, "Second quarter 2026 results");
}

#[tokio::test]
async fn get_disclosure__unknown_id__is_not_found() {
    let fixture = fixture().await;

    let result = get(&fixture, 7).await;

    assert_eq!(code(result), Code::NotFound);
}

#[tokio::test]
async fn get_disclosure__test_disclosure__returns_it() {
    let fixture = fixture().await;
    let test = with(message(7), "test", json!(true));
    fixture
        .store
        .store_message(&fetched(&test, true), NOW_NS)
        .await
        .unwrap();

    let disclosure = get(&fixture, 7).await.unwrap();

    assert!(disclosure.test);
}

// ReadAttachment

async fn read(
    fixture: &Fixture,
    attachment_id: i64,
) -> Result<Vec<pb::ReadAttachmentResponse>, Status> {
    let request = Request::new(pb::ReadAttachmentRequest {
        message_id: 1,
        attachment_id,
    });
    let response = fixture.service.read_attachment(request).await?;
    Ok(all(response.into_inner()).await)
}

/// A fixture with disclosure 1, read in full, with pending attachment 9.
async fn fixture_with_attachment() -> Fixture {
    let fixture = fixture().await;
    let full = fetched(&with_attachments(message(1), &[9]), true);
    fixture.store.store_message(&full, NOW_NS).await.unwrap();
    fixture
}

#[tokio::test]
async fn read_attachment__stored_file_larger_than_a_chunk__chunks_join_to_the_file() {
    let fixture = fixture_with_attachment().await;
    let bytes: Vec<u8> = (0..2 * CHUNK_SIZE + 100).map(|i| (i % 251) as u8).collect();
    let file = fixture
        .files
        .put(1, 9, Bytes::from(bytes.clone()))
        .await
        .unwrap();
    fixture
        .store
        .mark_attachment_stored(1, 9, &file, NOW_NS)
        .await
        .unwrap();

    let chunks = read(&fixture, 9).await.unwrap();

    let joined: Vec<u8> = chunks.into_iter().flat_map(|c| c.chunk).collect();
    assert_eq!(joined, bytes);
}

#[tokio::test]
async fn read_attachment__pending__is_failed_precondition() {
    let fixture = fixture_with_attachment().await;

    let result = read(&fixture, 9).await;

    assert_eq!(code(result), Code::FailedPrecondition);
}

#[tokio::test]
async fn read_attachment__unknown_attachment__is_not_found() {
    let fixture = fixture_with_attachment().await;

    let result = read(&fixture, 10).await;

    assert_eq!(code(result), Code::NotFound);
}

// WatchEvents

#[tokio::test]
async fn watch_events__after_a_seq__replays_the_later_events_in_order() {
    let fixture = fixture().await;
    store_messages(&fixture.store, 1..=4).await;

    let response = fixture.service.watch_events(watch(1)).await.unwrap();
    let events = take(response.into_inner(), 3).await;

    assert_eq!(seqs(&events), [2, 3, 4]);
}

#[tokio::test]
async fn watch_events__since_seq_beyond_the_latest__is_out_of_range() {
    let fixture = fixture().await;
    store_messages(&fixture.store, 1..=2).await;

    let result = fixture.service.watch_events(watch(3)).await;

    assert_eq!(code(result), Code::OutOfRange);
}

#[tokio::test]
async fn watch_events__events_written_after_the_replay__delivers_each_in_order() {
    let fixture = fixture().await;
    store_messages(&fixture.store, [1]).await;
    let response = fixture.service.watch_events(watch(0)).await.unwrap();
    let mut stream = response.into_inner();
    let replayed = take(&mut stream, 1).await;

    store_messages(&fixture.store, 2..=4).await;
    let live = take(&mut stream, 3).await;

    assert_eq!(seqs(&[replayed, live].concat()), [1, 2, 3, 4]);
}

#[tokio::test]
async fn watch_events__events_written_during_the_replay__delivers_each_once_in_order() {
    let fixture = fixture().await;
    let before = 1_100;
    let during = 200;
    store_messages(&fixture.store, 1..=before).await;
    let response = fixture.service.watch_events(watch(0)).await.unwrap();
    let writer = tokio::spawn({
        let store = Arc::clone(&fixture.store);
        async move { store_messages(&store, before + 1..=before + during).await }
    });

    let events = take(response.into_inner(), (before + during) as usize).await;
    writer.await.unwrap();

    assert_eq!(seqs(&events), (1..=before + during).collect::<Vec<_>>());
}

// GetStatus

async fn status(fixture: &Fixture) -> pb::Status {
    let request = Request::new(pb::GetStatusRequest {});
    let response = fixture.service.get_status(request).await.unwrap();
    response.into_inner().status.unwrap()
}

#[tokio::test]
async fn get_status__ingest_disabled__lists_no_jobs() {
    let fixture = fixture().await;

    let status = status(&fixture).await;

    assert_eq!(status.jobs, []);
}

#[tokio::test]
async fn get_status__ingest_enabled__lists_the_four_jobs_in_order() {
    let fixture = fixture_with(StatusSettings {
        ingest_enabled: true,
        lookback_days: 3,
    })
    .await;

    let status = status(&fixture).await;

    let names: Vec<&str> = status.jobs.iter().map(|job| job.name.as_str()).collect();
    assert_eq!(names, ["poller", "backfiller", "reader", "downloader"]);
}

fn sealed_day(day: Date) -> ListedDay {
    ListedDay {
        day,
        listed_at_ns: NOW_NS,
        rows: 10,
        overflow: false,
        undecodable: 0,
        sealed: true,
    }
}

#[tokio::test]
async fn get_status__day_not_sealed__reports_it_as_a_gap() {
    let fixture = fixture().await;
    for day in [20, 21] {
        let listed = sealed_day(Date::constant(2026, 9, day));
        fixture.store.record_listed_day(&listed).await.unwrap();
    }

    let coverage = status(&fixture).await.coverage.unwrap();

    let gap = pb::DayRange {
        first: "2026-09-19".into(),
        last: "2026-09-19".into(),
    };
    assert_eq!(coverage.gaps, [gap]);
}

#[tokio::test]
async fn get_status__stored_disclosures_and_attachments__counts_them() {
    let fixture = fixture_with_attachment().await;
    let full = fetched(&with_attachments(message(2), &[5, 6]), true);
    fixture.store.store_message(&full, NOW_NS).await.unwrap();
    let file = fixture
        .files
        .put(2, 5, Bytes::from_static(b"%PDF"))
        .await
        .unwrap();
    fixture
        .store
        .mark_attachment_stored(2, 5, &file, NOW_NS)
        .await
        .unwrap();
    fixture
        .store
        .mark_attachment_failed(2, 6, NOW_NS)
        .await
        .unwrap();
    fixture
        .store
        .store_list_entry(&fetched(&message(3), false), NOW_NS)
        .await
        .unwrap();

    let status = status(&fixture).await;

    let expected = pb::Counts {
        disclosures: 3,
        disclosures_unread: 1,
        disclosures_failed: 0,
        attachments_pending: 1,
        attachments_stored: 1,
        attachments_failed: 1,
    };
    assert_eq!(status.counts, Some(expected));
}

// ListFailures

/// A fixture whose failure log holds, in order: a read failure of 1, one of
/// 2, and a download failure of attachment 9 of 1.
async fn fixture_with_failures() -> Fixture {
    let fixture = fixture_with_attachment().await;
    let store = &fixture.store;
    store
        .store_list_entry(&fetched(&message(2), false), NOW_NS)
        .await
        .unwrap();
    let failure = Failure {
        permanent: false,
        detail: "HTTP 503".into(),
    };
    store
        .record_read_failure(1, &failure, NOW_NS)
        .await
        .unwrap();
    store
        .record_read_failure(2, &failure, NOW_NS)
        .await
        .unwrap();
    store
        .record_attachment_failure(1, 9, &failure, NOW_NS)
        .await
        .unwrap();
    fixture
}

async fn failure_ids(fixture: &Fixture, message_id: i64) -> Vec<i64> {
    let request = Request::new(pb::ListFailuresRequest { message_id });
    let response = fixture.service.list_failures(request).await.unwrap();
    let failures = all(response.into_inner()).await;
    failures
        .iter()
        .map(|response| response.failure.as_ref().unwrap().id)
        .collect()
}

#[tokio::test]
async fn list_failures__every_failure__returns_newest_first() {
    let fixture = fixture_with_failures().await;

    let ids = failure_ids(&fixture, 0).await;

    assert_eq!(ids, [3, 2, 1]);
}

#[tokio::test]
async fn list_failures__one_disclosure__returns_only_its_failures() {
    let fixture = fixture_with_failures().await;

    let ids = failure_ids(&fixture, 1).await;

    assert_eq!(ids, [3, 1]);
}
