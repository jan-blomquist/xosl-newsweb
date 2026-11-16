//! Integration tests of the webhook: `run` against a store in memory and a
//! fake POST that records each request and fails as a test scripts it.

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use xosl_newsweb::exchange::{self, Fetched};
use xosl_newsweb::store::Store;
use xosl_newsweb::webhook::{
    self, Delivery, POSITION_NAME, Post, PostError, SEQ_HEADER, WebhookSettings,
};

/// 2026-09-22 12:01 UTC.
const NOW_NS: i64 = 1_790_078_460_000_000_000;
/// Long enough for any of these scenarios; it only keeps a broken test from
/// hanging.
const TIMEOUT: Duration = Duration::from_secs(30);
const SHORT_RETRY_DELAY: Duration = Duration::from_millis(2);
const LONG_RETRY_DELAY: Duration = Duration::from_secs(3600);
/// A number of failures no test reaches.
const ALWAYS: u32 = u32::MAX;

/// A message in the exchange's format, published at noon on 2026-09-22 UTC.
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

fn fetched(message: &Value) -> Fetched {
    let document = message.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, true).unwrap(),
        document,
    }
}

/// Stores the messages read in full; each writes one `DISCOVERED` event, so
/// on an empty store message `n` has seq `n`.
async fn store_messages(store: &Store, message_ids: impl IntoIterator<Item = i64>) {
    for message_id in message_ids {
        store
            .store_message(&fetched(&message(message_id)), NOW_NS)
            .await
            .unwrap();
    }
}

/// A POST that records each request and fails a scripted number of times per
/// seq with HTTP 503.
#[derive(Clone)]
struct FakePost {
    received: Arc<Mutex<Vec<Delivery>>>,
    failures_left: Arc<Mutex<HashMap<i64, u32>>>,
    count: Arc<watch::Sender<usize>>,
}

impl FakePost {
    fn new() -> Self {
        Self {
            received: Arc::default(),
            failures_left: Arc::default(),
            count: Arc::new(watch::Sender::new(0)),
        }
    }

    /// Fails the next `times` requests of the event `seq`.
    fn fail(&self, seq: i64, times: u32) {
        self.failures_left.lock().unwrap().insert(seq, times);
    }

    /// The seq of each request received, in order.
    fn received_seqs(&self) -> Vec<i64> {
        self.received.lock().unwrap().iter().map(seq_of).collect()
    }

    /// Returns once `count` requests have been received in all.
    async fn wait_for(&self, count: usize) {
        let mut receiver = self.count.subscribe();
        tokio::time::timeout(TIMEOUT, receiver.wait_for(|received| *received >= count))
            .await
            .expect("the requests did not arrive in time")
            .unwrap();
    }
}

impl Post for FakePost {
    async fn post(&self, request: &Delivery) -> Result<(), PostError> {
        let seq = seq_of(request);
        self.received.lock().unwrap().push(request.clone());
        self.count.send_modify(|count| *count += 1);
        let mut failures_left = self.failures_left.lock().unwrap();
        match failures_left.get_mut(&seq) {
            Some(left) if *left > 0 => {
                *left -= 1;
                Err(PostError::Status(503))
            }
            _ => Ok(()),
        }
    }
}

fn seq_of(request: &Delivery) -> i64 {
    let (_, value) = request
        .headers
        .iter()
        .find(|(name, _)| *name == SEQ_HEADER)
        .unwrap();
    value.parse().unwrap()
}

struct Running {
    shutdown: CancellationToken,
    task: JoinHandle<()>,
}

/// Starts `run` and returns once it has stored its start position, so an
/// event written afterwards is after that position.
async fn start(store: &Arc<Store>, post: &FakePost, first_retry_delay: Duration) -> Running {
    let shutdown = CancellationToken::new();
    let settings = WebhookSettings {
        url: "https://receiver.example/hook".into(),
        secret: None,
        first_retry_delay,
    };
    let task = tokio::spawn({
        let store = Arc::clone(store);
        let post = post.clone();
        let shutdown = shutdown.clone();
        async move { webhook::run(store, post, settings, &shutdown).await }
    });
    tokio::time::timeout(TIMEOUT, async {
        while store
            .delivery_position(POSITION_NAME)
            .await
            .unwrap()
            .is_none()
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("the webhook did not store its start position in time");
    Running { shutdown, task }
}

/// Cancels `run`; `true` when it returned within `TIMEOUT`.
async fn stop(running: Running) -> bool {
    running.shutdown.cancel();
    tokio::time::timeout(TIMEOUT, running.task).await.is_ok()
}

async fn empty_store() -> Arc<Store> {
    Arc::new(Store::open_in_memory().await.unwrap())
}

#[tokio::test]
async fn run__event_written_after_start__is_delivered() {
    let store = empty_store().await;
    let post = FakePost::new();
    let running = start(&store, &post, SHORT_RETRY_DELAY).await;

    store_messages(&store, [1]).await;
    post.wait_for(1).await;
    stop(running).await;

    assert_eq!(post.received_seqs(), [1]);
}

#[tokio::test]
async fn run__three_events__are_delivered_in_seq_order() {
    let store = empty_store().await;
    let post = FakePost::new();
    let running = start(&store, &post, SHORT_RETRY_DELAY).await;

    store_messages(&store, [1, 2, 3]).await;
    post.wait_for(3).await;
    stop(running).await;

    assert_eq!(post.received_seqs(), [1, 2, 3]);
}

#[tokio::test]
async fn run__events_before_the_first_start__are_not_delivered() {
    let store = empty_store().await;
    store_messages(&store, [1, 2]).await;
    let post = FakePost::new();
    let running = start(&store, &post, SHORT_RETRY_DELAY).await;

    store_messages(&store, [3]).await;
    post.wait_for(1).await;
    stop(running).await;

    assert_eq!(post.received_seqs(), [3]);
}

#[tokio::test]
async fn run__restart__continues_after_the_last_delivered_event() {
    let store = empty_store().await;
    let post = FakePost::new();
    let first = start(&store, &post, SHORT_RETRY_DELAY).await;
    store_messages(&store, [1]).await;
    post.wait_for(1).await;
    stop(first).await;

    let second = start(&store, &post, SHORT_RETRY_DELAY).await;
    store_messages(&store, [2]).await;
    post.wait_for(2).await;
    stop(second).await;

    assert_eq!(post.received_seqs(), [1, 2]);
}

#[tokio::test]
async fn run__restart_after_a_failed_delivery__sends_that_event_again() {
    let store = empty_store().await;
    let post = FakePost::new();
    post.fail(1, 1);
    let first = start(&store, &post, LONG_RETRY_DELAY).await;
    store_messages(&store, [1]).await;
    post.wait_for(1).await;
    stop(first).await;

    let second = start(&store, &post, SHORT_RETRY_DELAY).await;
    post.wait_for(2).await;
    stop(second).await;

    assert_eq!(post.received_seqs(), [1, 1]);
}

#[tokio::test]
async fn run__delivery_fails_twice__is_sent_three_times_before_the_next_event() {
    let store = empty_store().await;
    let post = FakePost::new();
    post.fail(1, 2);
    let running = start(&store, &post, SHORT_RETRY_DELAY).await;

    store_messages(&store, [1, 2]).await;
    post.wait_for(4).await;
    stop(running).await;

    assert_eq!(post.received_seqs(), [1, 1, 1, 2]);
}

#[tokio::test]
async fn run__event_keeps_failing__later_event_is_not_sent() {
    let store = empty_store().await;
    let post = FakePost::new();
    post.fail(1, ALWAYS);
    let running = start(&store, &post, SHORT_RETRY_DELAY).await;

    store_messages(&store, [1, 2]).await;
    post.wait_for(3).await;
    stop(running).await;

    assert!(!post.received_seqs().contains(&2));
}

#[tokio::test]
async fn run__cancelled_while_waiting_to_retry__returns() {
    let store = empty_store().await;
    let post = FakePost::new();
    post.fail(1, ALWAYS);
    let running = start(&store, &post, LONG_RETRY_DELAY).await;
    store_messages(&store, [1]).await;
    post.wait_for(1).await;

    let returned = stop(running).await;

    assert!(returned);
}
