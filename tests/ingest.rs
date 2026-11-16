//! Integration tests of the ingest jobs: each pass, the loops and the `run`
//! command after the store is open. They use a fake exchange, a fake clock,
//! a database in memory and an object store in memory; one writes to a
//! temporary folder to make a write fail.
//!
//! The tests of one pass run on tokio's real clock with the fake clock set
//! to an instant. The tests of the loops run on tokio's paused clock with a
//! fake clock that follows it; see `paused_fixture` for how the database
//! runs there.

#![allow(non_snake_case)]

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use jiff::civil::Date;
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use xosl_newsweb::cli::{MirrorSettings, run_mirror};
use xosl_newsweb::clock::Clock;
use xosl_newsweb::exchange::client::{ClientError, Exchange, TransportError};
use xosl_newsweb::exchange::{self, Fetched, Page};
use xosl_newsweb::files::Files;
use xosl_newsweb::ingest::{JobSettings, Jobs};
use xosl_newsweb::pb::{DisclosureFilter, Event, attachment, event};
use xosl_newsweb::store::{FailureStage, ListedDay, Store};

const TODAY: Date = Date::constant(2026, 9, 23);
const YESTERDAY: Date = Date::constant(2026, 9, 22);
const TWO_DAYS_AGO: Date = Date::constant(2026, 9, 21);
/// 09:30 in Oslo on `TODAY`.
const PUBLISHED_TODAY: &str = "2026-09-23T07:30:00Z";
/// 09:30 in Oslo on `YESTERDAY`.
const PUBLISHED_YESTERDAY: &str = "2026-09-22T07:30:00Z";

const HOUR_NS: i64 = 3_600_000_000_000;
/// Beyond the longest delay before a retry, 24 hours.
const BEYOND_EVERY_RETRY_DELAY_NS: i64 = 25 * HOUR_NS;
const POLL_INTERVAL_SECS: u64 = 60;
const POLL_INTERVAL: Duration = Duration::from_secs(POLL_INTERVAL_SECS);
/// An interval far longer than the time a paused clock advances while the
/// jobs wait for the database.
const ONE_HOUR_SECS: u64 = 3_600;
const ONE_HOUR: Duration = Duration::from_secs(ONE_HOUR_SECS);

/// Invented bytes of an attachment, and their SHA-256 from FIPS 180-2.
const ABC: &[u8] = b"abc";
const SHA256_OF_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// An instant given as Oslo local time, such as `2026-09-23T10:00`.
fn oslo_ns(local: &str) -> i64 {
    let zoned: jiff::Zoned = format!("{local}[Europe/Oslo]").parse().unwrap();
    zoned.timestamp().as_nanosecond() as i64
}

/// 10:00 in Oslo on `TODAY`.
fn ten_in_the_morning() -> i64 {
    oslo_ns("2026-09-23T10:00")
}

// Test data: invented disclosures in the exchange's format.

fn message(message_id: i64, published_time: &str, attachment_ids: &[i64]) -> Value {
    let attachments: Vec<Value> = attachment_ids
        .iter()
        .map(|id| json!({"id": id, "name": format!("{id}.pdf")}))
        .collect();
    json!({
        "messageId": message_id,
        "issuerId": 4242, "issuerName": "Example Shipping ASA", "issuerSign": "EXSH",
        "publishedTime": published_time,
        "title": "Second quarter 2026 results",
        "body": "Revenue rose.",
        "category": [{"id": 1002}],
        "attachments": attachments,
    })
}

/// A row of the exchange's list: no body and no attachments.
fn list_row(message_id: i64, published_time: &str) -> Fetched {
    let mut row = message(message_id, published_time, &[]);
    let object = row.as_object_mut().unwrap();
    object.remove("body");
    object.remove("attachments");
    let document = row.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, false).unwrap(),
        document,
    }
}

fn full_message(message_id: i64, published_time: &str, attachment_ids: &[i64]) -> Fetched {
    let document = message(message_id, published_time, attachment_ids).to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, true).unwrap(),
        document,
    }
}

fn page(entries: Vec<Fetched>) -> Page {
    Page {
        entries,
        overflow: false,
        undecodable: 0,
    }
}

// The fake exchange.

/// What the fake exchange answers to one call.
#[derive(Debug, Clone)]
enum Reply<T> {
    Ok(T),
    Status(u16),
    Timeout,
}

/// The replies scripted for each day, message or attachment.
type Scripts<K, T> = Mutex<HashMap<K, VecDeque<Reply<T>>>>;

/// A call the fake exchange received.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Call {
    List(Date),
    Message(i64),
    Attachment(i64, i64),
}

/// Answers each call with the replies scripted for its day, message or
/// attachment, in order, and repeats the last one. Without a script it
/// answers as the exchange does for what does not exist: an empty list for a
/// day, a 404 for a message or an attachment. It records every call, and
/// when it arrived.
struct FakeExchange {
    lists: Scripts<Date, Page>,
    messages: Scripts<i64, Fetched>,
    attachments: Scripts<(i64, i64), Bytes>,
    calls: Mutex<Vec<(Instant, Call)>>,
    call_count: watch::Sender<usize>,
}

impl FakeExchange {
    fn new() -> Self {
        Self {
            lists: Mutex::default(),
            messages: Mutex::default(),
            attachments: Mutex::default(),
            calls: Mutex::default(),
            call_count: watch::Sender::new(0),
        }
    }

    fn with_list(self, day: Date, replies: Vec<Reply<Page>>) -> Self {
        self.lists.lock().unwrap().insert(day, replies.into());
        self
    }

    fn with_message(self, message_id: i64, replies: Vec<Reply<Fetched>>) -> Self {
        self.messages
            .lock()
            .unwrap()
            .insert(message_id, replies.into());
        self
    }

    fn with_attachment(
        self,
        message_id: i64,
        attachment_id: i64,
        replies: Vec<Reply<Bytes>>,
    ) -> Self {
        self.attachments
            .lock()
            .unwrap()
            .insert((message_id, attachment_id), replies.into());
        self
    }

    fn calls(&self) -> Vec<Call> {
        let calls = self.calls.lock().unwrap();
        calls.iter().map(|(_, call)| *call).collect()
    }

    fn message_calls(&self) -> usize {
        let calls = self.calls();
        calls
            .iter()
            .filter(|call| matches!(call, Call::Message(_)))
            .count()
    }

    /// When each call arrived, as whole seconds since `start`.
    fn call_seconds(&self, start: Instant) -> Vec<u64> {
        let calls = self.calls.lock().unwrap();
        calls
            .iter()
            .map(|(at, _)| (*at - start).as_secs())
            .collect()
    }

    async fn wait_for_calls(&self, count: usize) {
        let mut receiver = self.call_count.subscribe();
        receiver.wait_for(|calls| *calls >= count).await.unwrap();
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push((Instant::now(), call));
        self.call_count.send_modify(|count| *count += 1);
    }
}

/// The next scripted reply; the last one repeats.
fn next_reply<K: Eq + std::hash::Hash, T: Clone>(
    scripts: &Scripts<K, T>,
    key: &K,
) -> Option<Reply<T>> {
    let mut scripts = scripts.lock().unwrap();
    let replies = scripts.get_mut(key)?;
    if replies.len() > 1 {
        replies.pop_front()
    } else {
        replies.front().cloned()
    }
}

fn answer<T>(reply: Reply<T>, url: String) -> Result<T, ClientError> {
    match reply {
        Reply::Ok(value) => Ok(value),
        Reply::Status(code) => Err(ClientError::Transport {
            url,
            source: TransportError::Status(code),
        }),
        Reply::Timeout => Err(ClientError::Transport {
            url,
            source: TransportError::Timeout,
        }),
    }
}

impl Exchange for FakeExchange {
    async fn list(&self, day: Date) -> Result<Page, ClientError> {
        self.record(Call::List(day));
        let reply = next_reply(&self.lists, &day).unwrap_or(Reply::Ok(page(vec![])));
        answer(reply, exchange::list_url("https://api.example", day))
    }

    async fn message(&self, message_id: i64) -> Result<Fetched, ClientError> {
        self.record(Call::Message(message_id));
        let reply = next_reply(&self.messages, &message_id).unwrap_or(Reply::Status(404));
        answer(
            reply,
            exchange::message_url("https://api.example", message_id),
        )
    }

    async fn attachment(&self, message_id: i64, attachment_id: i64) -> Result<Bytes, ClientError> {
        self.record(Call::Attachment(message_id, attachment_id));
        let key = (message_id, attachment_id);
        let reply = next_reply(&self.attachments, &key).unwrap_or(Reply::Status(404));
        let url = exchange::attachment_url("https://api.example", message_id, attachment_id);
        answer(reply, url)
    }
}

// The fake clock.

/// A clock set to an instant. Made with `following_tokio`, it then advances
/// as tokio's clock does; otherwise it stays at the instant until set again.
struct FakeClock {
    set_ns: AtomicI64,
    /// When the clock was set, on tokio's clock, if it follows tokio's clock.
    set_at: Option<Mutex<Instant>>,
}

impl FakeClock {
    fn at(now_ns: i64) -> Self {
        Self {
            set_ns: AtomicI64::new(now_ns),
            set_at: None,
        }
    }

    fn following_tokio(now_ns: i64) -> Self {
        Self {
            set_ns: AtomicI64::new(now_ns),
            set_at: Some(Mutex::new(Instant::now())),
        }
    }

    fn set(&self, now_ns: i64) {
        if let Some(set_at) = &self.set_at {
            *set_at.lock().unwrap() = Instant::now();
        }
        self.set_ns.store(now_ns, Ordering::SeqCst);
    }

    fn advance_ns(&self, ns: i64) {
        self.set(self.now_ns() + ns);
    }
}

impl Clock for FakeClock {
    fn now_ns(&self) -> i64 {
        let elapsed = self
            .set_at
            .as_ref()
            .map_or(Duration::ZERO, |set_at| set_at.lock().unwrap().elapsed());
        self.set_ns.load(Ordering::SeqCst) + elapsed.as_nanos() as i64
    }
}

// The fixture.

struct Fixture {
    store: Store,
    files: Files,
    exchange: FakeExchange,
    clock: FakeClock,
    _ticker: Option<Ticker>,
}

impl Fixture {
    fn jobs(&self) -> Jobs<'_, FakeExchange, FakeClock> {
        Jobs {
            store: &self.store,
            files: &self.files,
            exchange: &self.exchange,
            clock: &self.clock,
        }
    }
}

/// A fixture with an empty store and files in memory, its clock set to
/// 10:00 in Oslo on `TODAY`.
async fn fixture(exchange: FakeExchange) -> Fixture {
    fixture_with_files(exchange, in_memory_files()).await
}

async fn fixture_with_files(exchange: FakeExchange, files: Files) -> Fixture {
    Fixture {
        store: Store::open_in_memory().await.unwrap(),
        files,
        exchange,
        clock: FakeClock::at(ten_in_the_morning()),
        _ticker: None,
    }
}

/// A fixture for a test on tokio's paused clock: its clock starts at 10:00
/// in Oslo on `TODAY` and follows tokio's clock.
///
/// While SQLite's thread runs a query, the runtime has no task to run, and a
/// paused clock advances to the next timer. A `Ticker` keeps a timer at most
/// 10 ms away, so the clock advances in steps of 10 ms while a job waits for
/// the database, and not straight to the next timer of a job or of sqlx.
/// How many steps one wait takes depends on the machine's load, so a test
/// that does database work between two calls asserts their times only to
/// within an interval much longer than such waits.
async fn paused_fixture(exchange: FakeExchange) -> Fixture {
    let ticker = Ticker::start();
    let store = Store::open_in_memory().await.unwrap();
    Fixture {
        store,
        files: in_memory_files(),
        exchange,
        clock: FakeClock::following_tokio(ten_in_the_morning()),
        _ticker: Some(ticker),
    }
}

/// A task that sleeps 10 ms on tokio's clock, again and again, until it is
/// dropped.
struct Ticker(tokio::task::JoinHandle<()>);

impl Ticker {
    fn start() -> Self {
        Self(tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }))
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn in_memory_files() -> Files {
    Files::open("memory:///", []).unwrap()
}

/// Files whose every write fails: the folder they write to is a file.
fn unwritable_files(parent: &tempfile::TempDir) -> Files {
    let folder = parent.path().join("attachments");
    let url = url::Url::from_directory_path(&folder).unwrap().to_string();
    let files = Files::open(&url, []).unwrap();
    std::fs::remove_dir(&folder).unwrap();
    std::fs::write(&folder, b"").unwrap();
    files
}

async fn store_list_rows(fixture: &Fixture, rows: &[Fetched]) {
    for row in rows {
        fixture
            .store
            .store_list_entry(row, fixture.clock.now_ns())
            .await
            .unwrap();
    }
}

async fn store_full_message(fixture: &Fixture, message: &Fetched) {
    fixture
        .store
        .store_message(message, fixture.clock.now_ns())
        .await
        .unwrap();
}

/// Runs `count` reader passes, with the clock set beyond every retry delay
/// before each one after the first.
async fn reader_passes(fixture: &Fixture, count: usize) {
    for pass in 0..count {
        if pass > 0 {
            fixture.clock.advance_ns(BEYOND_EVERY_RETRY_DELAY_NS);
        }
        fixture.jobs().reader_pass().await.unwrap();
    }
}

/// Runs `count` downloader passes, as `reader_passes` runs reader passes.
async fn downloader_passes(fixture: &Fixture, count: usize) {
    for pass in 0..count {
        if pass > 0 {
            fixture.clock.advance_ns(BEYOND_EVERY_RETRY_DELAY_NS);
        }
        fixture.jobs().downloader_pass().await.unwrap();
    }
}

async fn all_disclosures(store: &Store) -> Vec<xosl_newsweb::pb::Disclosure> {
    let filter = DisclosureFilter::default();
    store.disclosures(&filter, None, 100).await.unwrap()
}

async fn listed_days(store: &Store) -> Vec<ListedDay> {
    let from = Date::constant(2026, 1, 1);
    let to = Date::constant(2026, 12, 31);
    store.listed_days(from, to).await.unwrap()
}

async fn events(store: &Store) -> Vec<Event> {
    store.events_after(0, 100).await.unwrap()
}

async fn attachment_state(store: &Store, message_id: i64) -> xosl_newsweb::pb::Attachment {
    let disclosure = store.disclosure(message_id).await.unwrap().unwrap();
    disclosure.attachments[0].clone()
}

fn sealed(day: Date) -> ListedDay {
    ListedDay {
        day,
        listed_at_ns: ten_in_the_morning(),
        rows: 1,
        overflow: false,
        undecodable: 0,
        sealed: true,
    }
}

fn job_settings(lookback_days: u32) -> JobSettings {
    JobSettings {
        poll_interval: POLL_INTERVAL,
        lookback_days,
    }
}

/// Cancels `shutdown` once `duration` has passed on tokio's clock.
async fn cancel_after(shutdown: &CancellationToken, duration: Duration) {
    tokio::time::sleep(duration).await;
    shutdown.cancel();
}

/// Cancels `shutdown` once the first attachment of disclosure `message_id`
/// is stored, or once a day has passed on tokio's clock.
async fn cancel_once_stored(store: &Store, message_id: i64, shutdown: &CancellationToken) {
    let stored = attachment::State::Stored as i32;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(86_400) {
        let disclosure = store.disclosure(message_id).await.unwrap();
        let state = disclosure.and_then(|d| d.attachments.first().map(|a| a.state));
        if state == Some(stored) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
    shutdown.cancel();
}

/// Cancels `shutdown` once `exchange` has received `count` calls.
async fn cancel_after_calls(exchange: &FakeExchange, shutdown: &CancellationToken, count: usize) {
    exchange.wait_for_calls(count).await;
    shutdown.cancel();
}

// The poller.

#[tokio::test]
async fn poller_pass__ten_in_the_morning__lists_today_only() {
    let fixture = fixture(FakeExchange::new()).await;

    fixture.jobs().poller_pass().await.unwrap();

    assert_eq!(fixture.exchange.calls(), [Call::List(TODAY)]);
}

#[tokio::test]
async fn poller_pass__five_past_midnight_in_oslo__lists_the_new_day() {
    let fixture = fixture(FakeExchange::new()).await;
    // 22:05 UTC on the day before.
    fixture.clock.set(oslo_ns("2026-09-23T00:05"));

    fixture.jobs().poller_pass().await.unwrap();

    assert_eq!(fixture.exchange.calls(), [Call::List(TODAY)]);
}

#[tokio::test]
async fn poller_pass__listed_rows__stores_each_to_be_read_newest_first() {
    let rows = vec![
        list_row(2, PUBLISHED_TODAY),
        list_row(1, "2026-09-23T06:00:00Z"),
    ];
    let exchange = FakeExchange::new().with_list(TODAY, vec![Reply::Ok(page(rows))]);
    let fixture = fixture(exchange).await;

    fixture.jobs().poller_pass().await.unwrap();

    let unread = fixture
        .store
        .unread_disclosures(fixture.clock.now_ns(), 10)
        .await
        .unwrap();
    assert_eq!(unread, [2, 1]);
}

#[tokio::test]
async fn poller_pass__list_fails__writes_a_failure_row_for_today() {
    let exchange = FakeExchange::new().with_list(TODAY, vec![Reply::Status(503)]);
    let fixture = fixture(exchange).await;

    fixture.jobs().poller_pass().await.unwrap();

    let failures = fixture.store.failures(10).await.unwrap();
    let logged: Vec<_> = failures.iter().map(|f| (f.stage, f.day)).collect();
    assert_eq!(logged, [(FailureStage::List, Some(TODAY))]);
}

#[tokio::test]
async fn poller_pass__list_fails__stores_no_disclosure() {
    let exchange = FakeExchange::new().with_list(TODAY, vec![Reply::Status(503)]);
    let fixture = fixture(exchange).await;

    fixture.jobs().poller_pass().await.unwrap();

    assert!(all_disclosures(&fixture.store).await.is_empty());
}

#[tokio::test]
async fn poller_pass__listed_today__writes_no_listed_day() {
    let rows = vec![list_row(1, PUBLISHED_TODAY)];
    let exchange = FakeExchange::new().with_list(TODAY, vec![Reply::Ok(page(rows))]);
    let fixture = fixture(exchange).await;

    fixture.jobs().poller_pass().await.unwrap();

    assert!(listed_days(&fixture.store).await.is_empty());
}

#[tokio::test]
async fn poller_pass__second_pass_with_the_same_rows__writes_nothing_new() {
    let rows = vec![list_row(1, PUBLISHED_TODAY), list_row(2, PUBLISHED_TODAY)];
    let exchange = FakeExchange::new().with_list(TODAY, vec![Reply::Ok(page(rows))]);
    let fixture = fixture(exchange).await;
    fixture.jobs().poller_pass().await.unwrap();
    let after_first_pass = all_disclosures(&fixture.store).await;
    fixture.clock.advance_ns(HOUR_NS);

    fixture.jobs().poller_pass().await.unwrap();

    assert_eq!(all_disclosures(&fixture.store).await, after_first_pass);
}

// The backfiller.

#[tokio::test]
async fn backfiller_pass__clean_past_day__records_it_sealed() {
    let rows = vec![list_row(1, PUBLISHED_YESTERDAY)];
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Ok(page(rows))]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(1).await.unwrap();

    let expected = ListedDay {
        day: YESTERDAY,
        listed_at_ns: ten_in_the_morning(),
        rows: 1,
        overflow: false,
        undecodable: 0,
        sealed: true,
    };
    assert_eq!(listed_days(&fixture.store).await, [expected]);
}

#[tokio::test]
async fn backfiller_pass__past_day__stores_its_rows_to_be_read() {
    let rows = vec![list_row(1, PUBLISHED_YESTERDAY)];
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Ok(page(rows))]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(1).await.unwrap();

    let unread = fixture
        .store
        .unread_disclosures(fixture.clock.now_ns(), 10)
        .await
        .unwrap();
    assert_eq!(unread, [1]);
}

#[tokio::test]
async fn backfiller_pass__overflow__records_the_day_unsealed() {
    let truncated = Page {
        overflow: true,
        ..page(vec![list_row(1, PUBLISHED_YESTERDAY)])
    };
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Ok(truncated)]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(1).await.unwrap();

    let listed = listed_days(&fixture.store).await;
    assert_eq!((listed[0].overflow, listed[0].sealed), (true, false));
}

#[tokio::test]
async fn backfiller_pass__overflow__writes_a_failure_row_naming_the_truncation() {
    let truncated = Page {
        overflow: true,
        ..page(vec![list_row(1, PUBLISHED_YESTERDAY)])
    };
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Ok(truncated)]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(1).await.unwrap();

    let failures = fixture.store.failures(10).await.unwrap();
    assert!(failures[0].detail.contains("truncated"), "{failures:?}");
}

#[tokio::test]
async fn backfiller_pass__undecodable_row__records_the_day_unsealed() {
    let with_undecodable = Page {
        undecodable: 1,
        ..page(vec![list_row(1, PUBLISHED_YESTERDAY)])
    };
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Ok(with_undecodable)]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(1).await.unwrap();

    let listed = listed_days(&fixture.store).await;
    assert_eq!((listed[0].rows, listed[0].sealed), (2, false));
}

#[tokio::test]
async fn backfiller_pass__every_day_sealed__sends_no_request() {
    let fixture = fixture(FakeExchange::new()).await;
    fixture
        .store
        .record_listed_day(&sealed(YESTERDAY))
        .await
        .unwrap();

    fixture.jobs().backfiller_pass(1).await.unwrap();

    assert!(fixture.exchange.calls().is_empty());
}

#[tokio::test]
async fn backfiller_pass__listing_of_a_day_fails__ends_the_pass_without_listing_the_next_day() {
    let exchange = FakeExchange::new()
        .with_list(YESTERDAY, vec![Reply::Status(503)])
        .with_list(TWO_DAYS_AGO, vec![Reply::Ok(page(vec![]))]);
    let fixture = fixture(exchange).await;

    fixture.jobs().backfiller_pass(2).await.unwrap();

    let listed = listed_days(&fixture.store).await;
    assert!(listed.is_empty());
}

#[tokio::test]
async fn backfiller_pass__every_listing_fails__reports_no_day_listed() {
    let exchange = FakeExchange::new().with_list(YESTERDAY, vec![Reply::Status(503)]);
    let fixture = fixture(exchange).await;

    let listed = fixture.jobs().backfiller_pass(1).await.unwrap();

    assert_eq!(listed, 0);
}

// The reader.

#[tokio::test]
async fn reader_pass__listed_disclosure__reads_it_in_full() {
    let exchange =
        FakeExchange::new().with_message(1, vec![Reply::Ok(full_message(1, PUBLISHED_TODAY, &[]))]);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    fixture.jobs().reader_pass().await.unwrap();

    let disclosure = fixture.store.disclosure(1).await.unwrap().unwrap();
    assert!(disclosure.read_in_full);
}

#[tokio::test]
async fn reader_pass__listed_disclosure__writes_a_discovered_event() {
    let exchange =
        FakeExchange::new().with_message(1, vec![Reply::Ok(full_message(1, PUBLISHED_TODAY, &[]))]);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    fixture.jobs().reader_pass().await.unwrap();

    let kinds: Vec<i32> = events(&fixture.store)
        .await
        .iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(kinds, [event::Kind::Discovered as i32]);
}

#[tokio::test]
async fn reader_pass__todays_and_an_older_disclosure__requests_todays_first() {
    let exchange = FakeExchange::new()
        .with_message(
            1,
            vec![Reply::Ok(full_message(1, PUBLISHED_YESTERDAY, &[]))],
        )
        .with_message(2, vec![Reply::Ok(full_message(2, PUBLISHED_TODAY, &[]))]);
    let fixture = fixture(exchange).await;
    let rows = [
        list_row(1, PUBLISHED_YESTERDAY),
        list_row(2, PUBLISHED_TODAY),
    ];
    store_list_rows(&fixture, &rows).await;

    fixture.jobs().reader_pass().await.unwrap();

    assert_eq!(
        fixture.exchange.calls(),
        [Call::Message(2), Call::Message(1)]
    );
}

#[tokio::test]
async fn reader_pass__transport_outage_over_many_passes__never_marks_it_failed() {
    let mut replies = vec![Reply::Timeout; 10];
    replies.push(Reply::Ok(full_message(1, PUBLISHED_TODAY, &[])));
    let exchange = FakeExchange::new().with_message(1, replies);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    reader_passes(&fixture, 11).await;

    let disclosure = fixture.store.disclosure(1).await.unwrap().unwrap();
    assert!(disclosure.read_in_full);
}

#[tokio::test]
async fn reader_pass__third_404__marks_it_failed() {
    let exchange = FakeExchange::new().with_message(1, vec![Reply::Status(404)]);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    reader_passes(&fixture, 3).await;

    let unread = fixture
        .store
        .unread_disclosures(i64::MAX, 10)
        .await
        .unwrap();
    assert!(unread.is_empty());
}

#[tokio::test]
async fn reader_pass__failed_after_three_404s__is_not_requested_again() {
    let exchange = FakeExchange::new().with_message(1, vec![Reply::Status(404)]);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    reader_passes(&fixture, 5).await;

    assert_eq!(fixture.exchange.message_calls(), 3);
}

#[tokio::test]
async fn reader_pass__queue_of_two__returns_two_tried() {
    let exchange =
        FakeExchange::new().with_message(1, vec![Reply::Ok(full_message(1, PUBLISHED_TODAY, &[]))]);
    let fixture = fixture(exchange).await;
    let rows = [list_row(1, PUBLISHED_TODAY), list_row(2, PUBLISHED_TODAY)];
    store_list_rows(&fixture, &rows).await;

    let tried = fixture.jobs().reader_pass().await.unwrap();

    assert_eq!(tried, 2);
}

#[tokio::test]
async fn reader_pass__exchange_answers_with_another_message__records_a_transient_failure() {
    let exchange =
        FakeExchange::new().with_message(1, vec![Reply::Ok(full_message(2, PUBLISHED_TODAY, &[]))]);
    let fixture = fixture(exchange).await;
    store_list_rows(&fixture, &[list_row(1, PUBLISHED_TODAY)]).await;

    fixture.jobs().reader_pass().await.unwrap();

    let failures = fixture.store.failures(10).await.unwrap();
    let logged: Vec<_> = failures.iter().map(|f| (f.stage, f.permanent)).collect();
    assert_eq!(logged, [(FailureStage::Read, false)]);
}

// The downloader.

/// A fixture whose store holds disclosure 1, read in full, with pending
/// attachment 9.
async fn fixture_with_pending_attachment(exchange: FakeExchange, files: Files) -> Fixture {
    let fixture = fixture_with_files(exchange, files).await;
    store_full_message(&fixture, &full_message(1, PUBLISHED_TODAY, &[9])).await;
    fixture
}

#[tokio::test]
async fn downloader_pass__pending_attachment__marks_it_stored_with_the_size_and_hash_of_the_bytes()
{
    let exchange =
        FakeExchange::new().with_attachment(1, 9, vec![Reply::Ok(Bytes::from_static(ABC))]);
    let fixture = fixture_with_pending_attachment(exchange, in_memory_files()).await;

    fixture.jobs().downloader_pass().await.unwrap();

    let stored = attachment_state(&fixture.store, 1).await;
    assert_eq!(
        (stored.state, stored.size_bytes, stored.sha256.as_str()),
        (attachment::State::Stored as i32, 3, SHA256_OF_ABC)
    );
}

#[tokio::test]
async fn downloader_pass__pending_attachment__stores_the_bytes() {
    let exchange =
        FakeExchange::new().with_attachment(1, 9, vec![Reply::Ok(Bytes::from_static(ABC))]);
    let fixture = fixture_with_pending_attachment(exchange, in_memory_files()).await;

    fixture.jobs().downloader_pass().await.unwrap();

    assert_eq!(fixture.files.get(1, 9).await.unwrap(), ABC);
}

#[tokio::test]
async fn downloader_pass__third_404__marks_it_failed() {
    let exchange = FakeExchange::new().with_attachment(1, 9, vec![Reply::Status(404)]);
    let fixture = fixture_with_pending_attachment(exchange, in_memory_files()).await;

    downloader_passes(&fixture, 3).await;

    let stored = attachment_state(&fixture.store, 1).await;
    assert_eq!(stored.state, attachment::State::Failed as i32);
}

#[tokio::test]
async fn downloader_pass__second_404__leaves_it_pending() {
    let exchange = FakeExchange::new().with_attachment(1, 9, vec![Reply::Status(404)]);
    let fixture = fixture_with_pending_attachment(exchange, in_memory_files()).await;

    downloader_passes(&fixture, 2).await;

    let stored = attachment_state(&fixture.store, 1).await;
    assert_eq!(stored.state, attachment::State::Pending as i32);
}

#[tokio::test]
async fn downloader_pass__write_of_the_file_fails__records_a_transient_failure() {
    let parent = tempfile::tempdir().unwrap();
    let exchange =
        FakeExchange::new().with_attachment(1, 9, vec![Reply::Ok(Bytes::from_static(ABC))]);
    let fixture = fixture_with_pending_attachment(exchange, unwritable_files(&parent)).await;

    fixture.jobs().downloader_pass().await.unwrap();

    let failures = fixture.store.failures(10).await.unwrap();
    let logged: Vec<_> = failures
        .iter()
        .map(|f| (f.stage, f.permanent, f.attachment_id))
        .collect();
    assert_eq!(logged, [(FailureStage::Attachment, false, 9)]);
}

// The loops.

#[tokio::test(start_paused = true)]
async fn run_poller__paused_clock__lists_once_per_interval() {
    let fixture = paused_fixture(FakeExchange::new()).await;
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();
    let start = Instant::now();

    tokio::join!(
        jobs.run_poller(POLL_INTERVAL, &shutdown),
        cancel_after_calls(&fixture.exchange, &shutdown, 3),
    );

    assert_eq!(
        fixture.exchange.call_seconds(start),
        [0, POLL_INTERVAL_SECS, 2 * POLL_INTERVAL_SECS]
    );
}

#[tokio::test(start_paused = true)]
async fn run_poller__list_fails__lists_again_one_interval_later() {
    let exchange =
        FakeExchange::new().with_list(TODAY, vec![Reply::Status(503), Reply::Ok(page(vec![]))]);
    let fixture = paused_fixture(exchange).await;
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();
    let start = Instant::now();

    tokio::join!(
        jobs.run_poller(POLL_INTERVAL, &shutdown),
        cancel_after_calls(&fixture.exchange, &shutdown, 2),
    );

    assert_eq!(
        fixture.exchange.call_seconds(start),
        [0, POLL_INTERVAL_SECS]
    );
}

#[tokio::test(start_paused = true)]
async fn run_reader__more_disclosures_than_one_batch__reads_them_all_without_waiting() {
    let mut exchange = FakeExchange::new();
    let mut rows = Vec::new();
    for message_id in 1..=25 {
        let message = full_message(message_id, PUBLISHED_TODAY, &[]);
        exchange = exchange.with_message(message_id, vec![Reply::Ok(message)]);
        rows.push(list_row(message_id, PUBLISHED_TODAY));
    }
    let fixture = paused_fixture(exchange).await;
    store_list_rows(&fixture, &rows).await;
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();
    let start = Instant::now();

    tokio::join!(
        jobs.run_reader(ONE_HOUR, &shutdown),
        cancel_after_calls(&fixture.exchange, &shutdown, 25),
    );

    let last_call = *fixture.exchange.call_seconds(start).last().unwrap();
    assert!(last_call < ONE_HOUR_SECS, "last call at {last_call} s");
}

#[tokio::test(start_paused = true)]
async fn run__cancelled__stops_every_job() {
    let fixture = paused_fixture(FakeExchange::new()).await;
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();
    let settings = job_settings(7);
    let run = jobs.run(&settings, &shutdown);
    tokio::pin!(run);
    tokio::select! {
        () = &mut run => {}
        () = fixture.exchange.wait_for_calls(1) => {}
    }

    shutdown.cancel();

    let stopped = tokio::time::timeout(Duration::from_secs(1), run).await;
    assert!(stopped.is_ok());
}

// The run command after the store is open.

#[tokio::test(start_paused = true)]
async fn run_mirror__ingest_off__sends_no_request() {
    let exchange = FakeExchange::new().with_list(
        TODAY,
        vec![Reply::Ok(page(vec![list_row(1, PUBLISHED_TODAY)]))],
    );
    let fixture = paused_fixture(exchange).await;
    let settings = MirrorSettings {
        ingest: false,
        jobs: job_settings(1),
    };
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();

    let (result, ()) = tokio::join!(
        run_mirror(&jobs, &settings, &shutdown),
        cancel_after(&shutdown, 2 * POLL_INTERVAL),
    );

    result.unwrap();
    assert!(fixture.exchange.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn run_mirror__ingest_on_with_one_disclosure_and_attachment__reads_it_and_stores_the_attachment()
 {
    let exchange = FakeExchange::new()
        .with_list(
            TODAY,
            vec![Reply::Ok(page(vec![list_row(1, PUBLISHED_TODAY)]))],
        )
        .with_message(1, vec![Reply::Ok(full_message(1, PUBLISHED_TODAY, &[9]))])
        .with_attachment(1, 9, vec![Reply::Ok(Bytes::from_static(ABC))]);
    let fixture = paused_fixture(exchange).await;
    let settings = MirrorSettings {
        ingest: true,
        jobs: job_settings(1),
    };
    let shutdown = CancellationToken::new();
    let jobs = fixture.jobs();

    let (result, ()) = tokio::join!(
        run_mirror(&jobs, &settings, &shutdown),
        cancel_once_stored(&fixture.store, 1, &shutdown),
    );

    result.unwrap();
    let disclosure = fixture.store.disclosure(1).await.unwrap().unwrap();
    assert_eq!(
        (disclosure.read_in_full, disclosure.attachments[0].state),
        (true, attachment::State::Stored as i32)
    );
}
