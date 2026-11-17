//! Integration tests of the store: they run the schema and every query
//! against SQLite itself. Most use a database in memory. Those that read
//! columns no call of the store returns, and the last one, write a file in a
//! temporary folder.

#![allow(non_snake_case)]

use jiff::civil::Date;
use serde_json::{Value, json};
use xosl_newsweb::exchange::{self, Fetched};
use xosl_newsweb::pb::{DisclosureFilter, attachment, event};
use xosl_newsweb::store::{
    AttachmentRef, Counts, Failure, FailureRecord, FailureStage, ListedDay, Position, QueueSizes,
    Retry, Store, StoredFile,
};

const NOON_NS: i64 = 1_790_078_400_000_000_000;
const NOW_NS: i64 = NOON_NS + 60_000_000_000;
const LATER_NS: i64 = NOW_NS + 60_000_000_000;
/// The first retry delay after a failure at `NOW_NS`: one minute.
const ONE_MINUTE_AFTER_NOW_NS: i64 = NOW_NS + 60_000_000_000;
/// Within the first retry delay, one minute, of a failure at `NOW_NS`.
const HALF_A_MINUTE_LATER_NS: i64 = NOW_NS + 30_000_000_000;
/// Beyond every retry delay of a failure at `NOW_NS`.
const NEXT_YEAR_NS: i64 = NOW_NS + 400 * 86_400_000_000_000;

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

fn full(message: &Value) -> Fetched {
    let document = message.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, true).unwrap(),
        document,
    }
}

fn listed(message: &Value) -> Fetched {
    let document = message.to_string();
    Fetched {
        disclosure: exchange::decode_document(&document, false).unwrap(),
        document,
    }
}

fn with_attachments(message: Value, attachment_ids: &[i64]) -> Value {
    let attachments: Vec<Value> = attachment_ids
        .iter()
        .map(|id| json!({"id": id, "name": format!("{id}.pdf")}))
        .collect();
    with(message, "attachments", json!(attachments))
}

fn published_at(message: Value, time: &str) -> Value {
    with(message, "publishedTime", json!(time))
}

fn retitled(message: Value) -> Value {
    with(
        message,
        "title",
        json!("Second quarter 2026 results (updated)"),
    )
}

async fn store() -> Store {
    Store::open_in_memory().await.unwrap()
}

fn transient() -> Failure {
    Failure {
        permanent: false,
        detail: "HTTP 503".into(),
    }
}

fn permanent() -> Failure {
    Failure {
        permanent: true,
        detail: "HTTP 404".into(),
    }
}

async fn record_read_failures(store: &Store, message_id: i64, failure: &Failure, count: usize) {
    for _ in 0..count {
        store
            .record_read_failure(message_id, failure, NOW_NS)
            .await
            .unwrap();
    }
}

async fn record_attachment_failures(
    store: &Store,
    message_id: i64,
    attachment_id: i64,
    failure: &Failure,
    count: usize,
) {
    for _ in 0..count {
        store
            .record_attachment_failure(message_id, attachment_id, failure, NOW_NS)
            .await
            .unwrap();
    }
}

/// A store with one disclosure, read in full, with attachment 9.
async fn store_with_attachment() -> Store {
    let store = store().await;
    let message = with_attachments(message(1), &[9]);
    store.store_message(&full(&message), NOW_NS).await.unwrap();
    store
}

async fn attachment_state(store: &Store, message_id: i64) -> i32 {
    let disclosure = store.disclosure(message_id).await.unwrap().unwrap();
    disclosure.attachments[0].state
}

fn listed_day(day: Date, rows: u32) -> ListedDay {
    ListedDay {
        day,
        listed_at_ns: NOW_NS,
        rows,
        overflow: false,
        undecodable: 0,
        sealed: false,
    }
}

async fn kinds(store: &Store) -> Vec<i32> {
    let events = store.events_after(0, 100).await.unwrap();
    events.iter().map(|event| event.kind).collect()
}

const DISCOVERED: i32 = event::Kind::Discovered as i32;
const REVISED: i32 = event::Kind::Revised as i32;
const CORRECTED: i32 = event::Kind::Corrected as i32;
const ATTACHMENT_STORED: i32 = event::Kind::AttachmentStored as i32;
const ATTACHMENT_FAILED: i32 = event::Kind::AttachmentFailed as i32;

#[tokio::test]
async fn store_list_entry__new_disclosure__stores_it_without_an_event() {
    let store = store().await;

    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    let disclosure = store.disclosure(1).await.unwrap().unwrap();
    assert!(!disclosure.read_in_full);
    assert_eq!(disclosure.received_at_ns, NOW_NS);
    assert!(kinds(&store).await.is_empty());
}

#[tokio::test]
async fn store_list_entry__new_disclosure__returns_true() {
    let store = store().await;

    let new = store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    assert!(new);
}

#[tokio::test]
async fn store_list_entry__disclosure_already_stored__returns_false() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    let new = store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    assert!(!new);
}

#[tokio::test]
async fn store_list_entry__disclosure_already_read_in_full__keeps_the_full_message() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&message(1)), LATER_NS)
        .await
        .unwrap();

    let disclosure = store.disclosure(1).await.unwrap().unwrap();
    assert!(disclosure.read_in_full);
    assert_eq!(disclosure.body, "Revenue rose.");
}

#[tokio::test]
async fn store_list_entry__same_row_of_disclosure_read_in_full__leaves_it_out_of_the_unread_queue()
{
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&message(1)), LATER_NS)
        .await
        .unwrap();

    assert!(
        store
            .unread_disclosures(LATER_NS, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn store_list_entry__changed_row_of_list_row__replaces_the_stored_row() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    let disclosure = store.disclosure(1).await.unwrap().unwrap();
    assert_eq!(disclosure.title, "Second quarter 2026 results (updated)");
}

#[tokio::test]
async fn store_list_entry__publication_time_of_list_row_changed__lists_it_at_the_new_time() {
    let store = store().await;
    let at_eleven = published_at(message(1), "2026-09-22T11:00:00Z");
    store
        .store_list_entry(&listed(&at_eleven), NOW_NS)
        .await
        .unwrap();
    let filter = DisclosureFilter {
        published_from_ns: NOON_NS,
        ..Default::default()
    };

    store
        .store_list_entry(&listed(&message(1)), LATER_NS)
        .await
        .unwrap();

    let listed = store.disclosures(&filter, None, 10).await.unwrap();
    let ids: Vec<i64> = listed.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [1]);
}

#[tokio::test]
async fn store_list_entry__changed_row_of_disclosure_read_in_full__queues_it_to_be_read_again() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    assert_eq!(store.unread_disclosures(LATER_NS, 10).await.unwrap(), [1]);
}

#[tokio::test]
async fn store_list_entry__changed_row_of_disclosure_read_in_full__keeps_the_full_message() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    let disclosure = store.disclosure(1).await.unwrap().unwrap();
    assert_eq!(disclosure.title, "Second quarter 2026 results");
}

#[tokio::test]
async fn store_list_entry__changed_row__writes_no_event() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&message(2)), NOW_NS)
        .await
        .unwrap();

    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&retitled(message(2))), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED]);
}

#[tokio::test]
async fn store_message__changed_document_after_changed_list_row__writes_revised_event() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED, REVISED]);
}

#[tokio::test]
async fn store_message__unchanged_document_of_disclosure_queued_again__writes_no_event() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED]);
}

#[tokio::test]
async fn store_message__unchanged_document_of_disclosure_queued_again__removes_it_from_the_queue() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&retitled(message(1))), LATER_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();

    assert!(
        store
            .unread_disclosures(LATER_NS, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn store_message__after_transient_failures__lets_a_later_reread_start_at_once() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 1, &transient(), 2).await;
    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();
    let changed = with(
        message(1),
        "title",
        json!("Second quarter 2026 results (updated)"),
    );

    store
        .store_list_entry(&listed(&changed), LATER_NS)
        .await
        .unwrap();

    let queue = store.unread_disclosures(LATER_NS, 10).await.unwrap();
    assert_eq!(queue, [1]);
}

#[tokio::test]
async fn store_message__first_full_read__writes_one_discovered_event() {
    let store = store().await;
    let message = with(
        message(1),
        "attachments",
        json!([{"id": 9, "name": "q2.pdf"}]),
    );

    store.store_message(&full(&message), NOW_NS).await.unwrap();

    let events = store.events_after(0, 100).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, DISCOVERED);
    assert_eq!(events[0].at_ns, NOW_NS);
    let snapshot = events[0].disclosure.as_ref().unwrap();
    assert_eq!(
        snapshot.attachments[0].state,
        attachment::State::Pending as i32
    );
}

#[tokio::test]
async fn store_message__after_its_list_entry__keeps_received_time_and_writes_discovered() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();

    let disclosure = store.disclosure(1).await.unwrap().unwrap();
    assert!(disclosure.read_in_full);
    assert_eq!(disclosure.received_at_ns, NOW_NS);
    assert_eq!(kinds(&store).await, [DISCOVERED]);
}

#[tokio::test]
async fn store_message__same_document_again__writes_no_event() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED]);
}

#[tokio::test]
async fn store_message__changed_document__writes_revised_event_with_the_new_title() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    let revised = with(
        message(1),
        "title",
        json!("Second quarter 2026 results (updated)"),
    );

    store
        .store_message(&full(&revised), LATER_NS)
        .await
        .unwrap();

    let events = store.events_after(0, 100).await.unwrap();
    assert_eq!(events[1].kind, REVISED);
    let titles: Vec<&str> = events
        .iter()
        .map(|event| event.disclosure.as_ref().unwrap().title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Second quarter 2026 results",
            "Second quarter 2026 results (updated)"
        ]
    );
}

#[tokio::test]
async fn store_message__correction_arrives_second__writes_corrected_event_for_the_original() {
    let store = store().await;
    store
        .store_message(&full(&message(690)), NOW_NS)
        .await
        .unwrap();
    let correction = with(message(700), "correctionForMessageId", json!(690));

    store
        .store_message(&full(&correction), LATER_NS)
        .await
        .unwrap();

    let events = store.events_after(0, 100).await.unwrap();
    let corrected = events.iter().find(|event| event.kind == CORRECTED).unwrap();
    let snapshot = corrected.disclosure.as_ref().unwrap();
    assert_eq!(snapshot.message_id, 690);
    assert_eq!(snapshot.corrected_by_message_id, 700);
}

#[tokio::test]
async fn store_list_entry__correction_listed_after_original_was_read__writes_corrected_event() {
    let store = store().await;
    store
        .store_message(&full(&message(690)), NOW_NS)
        .await
        .unwrap();
    let correction = with(message(700), "correctionForMessageId", json!(690));

    store
        .store_list_entry(&listed(&correction), LATER_NS)
        .await
        .unwrap();
    store
        .store_message(&full(&correction), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED, CORRECTED, DISCOVERED]);
}

#[tokio::test]
async fn store_message__correction_arrives_first__writes_corrected_event_when_original_is_read() {
    let store = store().await;
    let correction = with(message(700), "correctionForMessageId", json!(690));
    store
        .store_message(&full(&correction), NOW_NS)
        .await
        .unwrap();

    store
        .store_message(&full(&message(690)), LATER_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED, DISCOVERED, CORRECTED]);
}

#[tokio::test]
async fn store_message__correction_of_unknown_message__writes_no_corrected_event() {
    let store = store().await;
    let correction = with(message(700), "correctionForMessageId", json!(690));

    store
        .store_message(&full(&correction), NOW_NS)
        .await
        .unwrap();

    assert_eq!(kinds(&store).await, [DISCOVERED]);
}

#[tokio::test]
async fn disclosure__unknown_id__returns_none() {
    let store = store().await;

    let disclosure = store.disclosure(1).await.unwrap();

    assert_eq!(disclosure, None);
}

#[tokio::test]
async fn disclosure__test_disclosure__returns_it() {
    let store = store().await;
    let test = with(message(1), "test", json!(true));
    store.store_message(&full(&test), NOW_NS).await.unwrap();

    let disclosure = store.disclosure(1).await.unwrap();

    assert!(disclosure.unwrap().test);
}

#[tokio::test]
async fn disclosures__test_disclosure_in_range__omits_it() {
    let store = store().await;
    let test = with(message(1), "test", json!(true));
    store.store_message(&full(&test), NOW_NS).await.unwrap();
    store
        .store_message(&full(&message(2)), NOW_NS)
        .await
        .unwrap();

    let listed = store
        .disclosures(&DisclosureFilter::default(), None, 10)
        .await
        .unwrap();

    let ids: Vec<i64> = listed.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [2]);
}

#[tokio::test]
async fn disclosures__equal_publication_times__orders_by_greater_id_first() {
    let store = store().await;
    let earlier = with(message(3), "publishedTime", json!("2026-09-22T11:00:00Z"));
    for message in [message(1), message(2), earlier] {
        store.store_message(&full(&message), NOW_NS).await.unwrap();
    }

    let listed = store
        .disclosures(&DisclosureFilter::default(), None, 10)
        .await
        .unwrap();

    let ids: Vec<i64> = listed.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [2, 1, 3]);
}

#[tokio::test]
async fn disclosures__issuer_signs__returns_only_those_signs() {
    let store = store().await;
    let other = with(message(2), "issuerSign", json!("OTHR"));
    for message in [message(1), other] {
        store.store_message(&full(&message), NOW_NS).await.unwrap();
    }
    let filter = DisclosureFilter {
        issuer_signs: vec!["OTHR".into(), "NONE".into()],
        ..Default::default()
    };

    let listed = store.disclosures(&filter, None, 10).await.unwrap();

    let ids: Vec<i64> = listed.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [2]);
}

#[tokio::test]
async fn disclosures__time_range__includes_from_and_excludes_before() {
    let store = store().await;
    let hour_before = with(message(1), "publishedTime", json!("2026-09-22T11:00:00Z"));
    let hour_after = with(message(3), "publishedTime", json!("2026-09-22T13:00:00Z"));
    for message in [hour_before, message(2), hour_after] {
        store.store_message(&full(&message), NOW_NS).await.unwrap();
    }
    let filter = DisclosureFilter {
        published_from_ns: NOON_NS,
        published_before_ns: NOON_NS + 3_600_000_000_000,
        ..Default::default()
    };

    let listed = store.disclosures(&filter, None, 10).await.unwrap();

    let ids: Vec<i64> = listed.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [2]);
}

#[tokio::test]
async fn disclosures__after_a_position__continues_with_the_next_disclosure() {
    let store = store().await;
    for message in [message(1), message(2), message(3)] {
        store.store_message(&full(&message), NOW_NS).await.unwrap();
    }
    let filter = DisclosureFilter::default();
    let first = store.disclosures(&filter, None, 2).await.unwrap();

    let rest = store
        .disclosures(&filter, first.last().map(Position::of), 2)
        .await
        .unwrap();

    let ids: Vec<i64> = rest.iter().map(|d| d.message_id).collect();
    assert_eq!(ids, [1]);
}

#[tokio::test]
async fn disclosures__corrected_disclosure__includes_it_with_corrected_by_message_id() {
    let store = store().await;
    store
        .store_message(&full(&message(690)), NOW_NS)
        .await
        .unwrap();
    let correction = with(message(700), "correctionForMessageId", json!(690));
    store
        .store_message(&full(&correction), NOW_NS)
        .await
        .unwrap();

    let listed = store
        .disclosures(&DisclosureFilter::default(), None, 10)
        .await
        .unwrap();

    let original = listed.iter().find(|d| d.message_id == 690).unwrap();
    assert_eq!(original.corrected_by_message_id, 700);
}

#[tokio::test]
async fn mark_attachment_stored__pending_attachment__sets_its_state_and_writes_event() {
    let store = store().await;
    let message = with(
        message(1),
        "attachments",
        json!([{"id": 9, "name": "q2.pdf"}]),
    );
    store.store_message(&full(&message), NOW_NS).await.unwrap();
    let file = StoredFile {
        content_type: "application/pdf".into(),
        size_bytes: 84_743,
        sha256: "ab".repeat(32),
    };

    store
        .mark_attachment_stored(1, 9, &file, LATER_NS)
        .await
        .unwrap();

    let attachment = store
        .disclosure(1)
        .await
        .unwrap()
        .unwrap()
        .attachments
        .remove(0);
    assert_eq!(attachment.state, attachment::State::Stored as i32);
    assert_eq!(attachment.name, "q2.pdf");
    assert_eq!(attachment.size_bytes, 84_743);
    let events = store.events_after(1, 100).await.unwrap();
    assert_eq!(events[0].kind, ATTACHMENT_STORED);
    assert_eq!(events[0].attachment_id, 9);
}

#[tokio::test]
async fn mark_attachment_failed__pending_attachment__sets_its_state_and_writes_event() {
    let store = store().await;
    let message = with(
        message(1),
        "attachments",
        json!([{"id": 9, "name": "q2.pdf"}]),
    );
    store.store_message(&full(&message), NOW_NS).await.unwrap();

    store.mark_attachment_failed(1, 9, LATER_NS).await.unwrap();

    let attachment = store
        .disclosure(1)
        .await
        .unwrap()
        .unwrap()
        .attachments
        .remove(0);
    assert_eq!(attachment.state, attachment::State::Failed as i32);
    assert_eq!(kinds(&store).await, [DISCOVERED, ATTACHMENT_FAILED]);
}

#[tokio::test]
async fn mark_attachment_stored__unknown_attachment__returns_error_naming_both_ids() {
    let store = store().await;
    let file = StoredFile {
        content_type: String::new(),
        size_bytes: 0,
        sha256: String::new(),
    };

    let error = store
        .mark_attachment_stored(1, 9, &file, NOW_NS)
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "message 1 has no attachment 9");
}

#[tokio::test]
async fn unread_disclosures__list_rows__returns_newest_first_then_greater_id_first() {
    let store = store().await;
    let at_eleven = published_at(message(3), "2026-09-22T11:00:00Z");
    let at_one = published_at(message(4), "2026-09-22T13:00:00Z");
    for message in [message(1), message(2), at_eleven, at_one] {
        store
            .store_list_entry(&listed(&message), NOW_NS)
            .await
            .unwrap();
    }

    let ids = store.unread_disclosures(NOW_NS, 10).await.unwrap();

    assert_eq!(ids, [4, 2, 1, 3]);
}

#[tokio::test]
async fn unread_disclosures__more_than_the_limit__returns_the_newest() {
    let store = store().await;
    let at_eleven = published_at(message(1), "2026-09-22T11:00:00Z");
    for message in [at_eleven, message(2)] {
        store
            .store_list_entry(&listed(&message), NOW_NS)
            .await
            .unwrap();
    }

    let ids = store.unread_disclosures(NOW_NS, 1).await.unwrap();

    assert_eq!(ids, [2]);
}

#[tokio::test]
async fn unread_disclosures__disclosure_read_in_full__leaves_it_out() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_list_entry(&listed(&message(2)), NOW_NS)
        .await
        .unwrap();

    let ids = store.unread_disclosures(NOW_NS, 10).await.unwrap();

    assert_eq!(ids, [2]);
}

#[tokio::test]
async fn record_read_failure__retry_time_in_the_future__keeps_it_out_of_the_queue() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap();

    let ids = store
        .unread_disclosures(HALF_A_MINUTE_LATER_NS, 10)
        .await
        .unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn record_read_failure__retry_time_reached__returns_it_to_the_queue() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(store.unread_disclosures(LATER_NS, 10).await.unwrap(), [1]);
}

#[tokio::test]
async fn record_read_failure__outage_of_fifty_transient_failures__never_marks_it_failed() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    record_read_failures(&store, 1, &transient(), 50).await;

    assert_eq!(
        store.unread_disclosures(NEXT_YEAR_NS, 10).await.unwrap(),
        [1]
    );
}

#[tokio::test]
async fn record_read_failure__second_permanent_failure__keeps_it_in_the_queue() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    record_read_failures(&store, 1, &permanent(), 2).await;

    assert_eq!(
        store.unread_disclosures(NEXT_YEAR_NS, 10).await.unwrap(),
        [1]
    );
}

#[tokio::test]
async fn record_read_failure__third_permanent_failure__removes_it_from_the_queue() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    record_read_failures(&store, 1, &permanent(), 3).await;

    let ids = store.unread_disclosures(NEXT_YEAR_NS, 10).await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn disclosure__read_failed_for_good__carries_read_failed() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 1, &permanent(), 3).await;

    let disclosure = store.disclosure(1).await.unwrap().unwrap();

    assert!(disclosure.read_failed);
}

#[tokio::test]
async fn record_read_failure__permanent_failure__retries_after_the_first_delay() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .record_read_failure(1, &permanent(), NOW_NS)
        .await
        .unwrap();

    let ids = store
        .unread_disclosures(HALF_A_MINUTE_LATER_NS, 10)
        .await
        .unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn record_read_failure__any_failure__appends_a_row_to_the_failure_log() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    store
        .record_read_failure(1, &permanent(), NOW_NS)
        .await
        .unwrap();

    let expected = FailureRecord {
        id: 1,
        at_ns: NOW_NS,
        stage: FailureStage::Read,
        permanent: true,
        message_id: 1,
        attachment_id: 0,
        day: None,
        detail: "HTTP 404".into(),
    };
    assert_eq!(store.failures(10).await.unwrap(), [expected]);
}

#[tokio::test]
async fn record_read_failure__unknown_disclosure__returns_error_naming_it() {
    let store = store().await;

    let error = store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "no disclosure 1 is stored");
}

#[tokio::test]
async fn pending_attachments__two_disclosures__returns_the_newest_disclosure_first() {
    let store = store().await;
    let at_eleven = with_attachments(published_at(message(1), "2026-09-22T11:00:00Z"), &[9]);
    let at_noon = with_attachments(message(2), &[20, 21]);
    for message in [at_eleven, at_noon] {
        store.store_message(&full(&message), NOW_NS).await.unwrap();
    }

    let pending = store.pending_attachments(NOW_NS, 10).await.unwrap();

    let expected = [
        AttachmentRef {
            message_id: 2,
            attachment_id: 20,
        },
        AttachmentRef {
            message_id: 2,
            attachment_id: 21,
        },
        AttachmentRef {
            message_id: 1,
            attachment_id: 9,
        },
    ];
    assert_eq!(pending, expected);
}

#[tokio::test]
async fn pending_attachments__stored_attachment__leaves_it_out() {
    let store = store_with_attachment().await;
    let file = StoredFile {
        content_type: "application/pdf".into(),
        size_bytes: 84_743,
        sha256: "ab".repeat(32),
    };
    store
        .mark_attachment_stored(1, 9, &file, NOW_NS)
        .await
        .unwrap();

    let pending = store.pending_attachments(NOW_NS, 10).await.unwrap();

    assert!(pending.is_empty());
}

#[tokio::test]
async fn record_attachment_failure__retry_time_in_the_future__keeps_it_out_of_the_queue() {
    let store = store_with_attachment().await;

    store
        .record_attachment_failure(1, 9, &transient(), NOW_NS)
        .await
        .unwrap();

    let pending = store
        .pending_attachments(HALF_A_MINUTE_LATER_NS, 10)
        .await
        .unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn record_attachment_failure__outage_of_fifty_transient_failures__keeps_it_pending() {
    let store = store_with_attachment().await;

    record_attachment_failures(&store, 1, 9, &transient(), 50).await;

    let pending = store.pending_attachments(NEXT_YEAR_NS, 10).await.unwrap();
    assert_eq!(pending.len(), 1);
}

#[tokio::test]
async fn record_attachment_failure__second_permanent_failure__keeps_it_pending() {
    let store = store_with_attachment().await;

    record_attachment_failures(&store, 1, 9, &permanent(), 2).await;

    assert_eq!(
        attachment_state(&store, 1).await,
        attachment::State::Pending as i32
    );
}

#[tokio::test]
async fn record_attachment_failure__third_permanent_failure__sets_its_state_to_failed() {
    let store = store_with_attachment().await;

    record_attachment_failures(&store, 1, 9, &permanent(), 3).await;

    assert_eq!(
        attachment_state(&store, 1).await,
        attachment::State::Failed as i32
    );
}

#[tokio::test]
async fn record_attachment_failure__third_permanent_failure__removes_it_from_the_queue() {
    let store = store_with_attachment().await;

    record_attachment_failures(&store, 1, 9, &permanent(), 3).await;

    let pending = store.pending_attachments(NEXT_YEAR_NS, 10).await.unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn record_attachment_failure__permanent_failures_beyond_the_third__write_one_failed_event() {
    let store = store_with_attachment().await;

    record_attachment_failures(&store, 1, 9, &permanent(), 5).await;

    assert_eq!(kinds(&store).await, [DISCOVERED, ATTACHMENT_FAILED]);
}

#[tokio::test]
async fn record_attachment_failure__any_failure__appends_a_row_to_the_failure_log() {
    let store = store_with_attachment().await;

    store
        .record_attachment_failure(1, 9, &transient(), NOW_NS)
        .await
        .unwrap();

    let expected = FailureRecord {
        id: 1,
        at_ns: NOW_NS,
        stage: FailureStage::Attachment,
        permanent: false,
        message_id: 1,
        attachment_id: 9,
        day: None,
        detail: "HTTP 503".into(),
    };
    assert_eq!(store.failures(10).await.unwrap(), [expected]);
}

#[tokio::test]
async fn record_attachment_failure__unknown_attachment__returns_error_naming_both_ids() {
    let store = store_with_attachment().await;

    let error = store
        .record_attachment_failure(1, 10, &transient(), NOW_NS)
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "message 1 has no attachment 10");
}

#[tokio::test]
async fn record_list_failure__one_day__appends_a_row_naming_the_day() {
    let store = store().await;

    store
        .record_list_failure(Date::constant(2026, 9, 22), "HTTP 503", NOW_NS)
        .await
        .unwrap();

    let expected = FailureRecord {
        id: 1,
        at_ns: NOW_NS,
        stage: FailureStage::List,
        permanent: false,
        message_id: 0,
        attachment_id: 0,
        day: Some(Date::constant(2026, 9, 22)),
        detail: "HTTP 503".into(),
    };
    assert_eq!(store.failures(10).await.unwrap(), [expected]);
}

#[tokio::test]
async fn failures__several__returns_newest_first_up_to_the_limit() {
    let store = store().await;
    for day in [21, 22, 23] {
        store
            .record_list_failure(Date::constant(2026, 9, day), "HTTP 503", NOW_NS)
            .await
            .unwrap();
    }

    let failures = store.failures(2).await.unwrap();

    let days: Vec<Option<Date>> = failures.iter().map(|failure| failure.day).collect();
    assert_eq!(
        days,
        [
            Some(Date::constant(2026, 9, 23)),
            Some(Date::constant(2026, 9, 22))
        ]
    );
}

#[tokio::test]
async fn record_listed_day__day_listed_again__replaces_its_row() {
    let store = store().await;
    let day = Date::constant(2026, 9, 22);
    store
        .record_listed_day(&listed_day(day, 200))
        .await
        .unwrap();
    let again = ListedDay {
        listed_at_ns: LATER_NS,
        rows: 214,
        overflow: true,
        undecodable: 1,
        sealed: true,
        ..listed_day(day, 0)
    };

    store.record_listed_day(&again).await.unwrap();

    assert_eq!(store.listed_days(day, day).await.unwrap(), [again]);
}

#[tokio::test]
async fn listed_days__range__returns_the_days_within_it_both_ends_included_ascending() {
    let store = store().await;
    for day in [24, 20, 21, 23, 25] {
        let listed = listed_day(Date::constant(2026, 9, day), 100);
        store.record_listed_day(&listed).await.unwrap();
    }

    let listed = store
        .listed_days(Date::constant(2026, 9, 21), Date::constant(2026, 9, 24))
        .await
        .unwrap();

    let days: Vec<Date> = listed.iter().map(|listed| listed.day).collect();
    assert_eq!(
        days,
        [
            Date::constant(2026, 9, 21),
            Date::constant(2026, 9, 23),
            Date::constant(2026, 9, 24)
        ]
    );
}

#[tokio::test]
async fn events_after__seq_of_first_event__returns_only_later_events() {
    let store = store().await;
    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .store_message(&full(&message(2)), LATER_NS)
        .await
        .unwrap();

    let events = store.events_after(1, 100).await.unwrap();

    let seqs: Vec<i64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, [2]);
    assert_eq!(store.latest_seq().await.unwrap(), 2);
}

#[tokio::test]
async fn latest_seq__empty_log__returns_zero() {
    let store = store().await;

    let seq = store.latest_seq().await.unwrap();

    assert_eq!(seq, 0);
}

#[tokio::test]
async fn open__existing_file__reads_what_an_earlier_store_wrote() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("mirror.sqlite");
    let earlier = Store::open(&path).await.unwrap();
    earlier
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();
    drop(earlier);

    let store = Store::open(&path).await.unwrap();

    assert!(store.disclosure(1).await.unwrap().is_some());
}

#[tokio::test]
async fn watch_latest_seq__existing_file__starts_at_its_latest_seq() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("mirror.sqlite");
    let earlier = Store::open(&path).await.unwrap();
    for message_id in [1, 2] {
        earlier
            .store_message(&full(&message(message_id)), NOW_NS)
            .await
            .unwrap();
    }
    drop(earlier);

    let store = Store::open(&path).await.unwrap();

    assert_eq!(*store.watch_latest_seq().borrow(), 2);
}

// The channel of the newest event's seq.

#[tokio::test]
async fn watch_latest_seq__first_full_read__holds_the_seq_of_its_event() {
    let store = store().await;
    let latest = store.watch_latest_seq();

    store
        .store_message(&full(&message(1)), NOW_NS)
        .await
        .unwrap();

    assert_eq!(*latest.borrow(), 1);
}

#[tokio::test]
async fn watch_latest_seq__write_without_an_event__is_not_changed() {
    let store = store().await;
    let latest = store.watch_latest_seq();

    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    assert!(!latest.has_changed().unwrap());
}

#[tokio::test]
async fn watch_latest_seq__events_of_each_kind__holds_the_newest_seq() {
    let store = store().await;
    let latest = store.watch_latest_seq();
    let attached = with_attachments(message(1), &[9, 10, 11]);
    store.store_message(&full(&attached), NOW_NS).await.unwrap();
    let file = StoredFile {
        content_type: "application/pdf".into(),
        size_bytes: 4,
        sha256: "00".into(),
    };
    store
        .mark_attachment_stored(1, 9, &file, NOW_NS)
        .await
        .unwrap();
    store.mark_attachment_failed(1, 10, NOW_NS).await.unwrap();

    record_attachment_failures(&store, 1, 11, &permanent(), 3).await;

    assert_eq!(*latest.borrow(), 4);
}

// The counts of what the store holds.

#[tokio::test]
async fn counts__disclosures_and_attachments_in_each_state__counts_each() {
    let store = store_with_attachment().await;
    let attached = with_attachments(message(2), &[5, 6]);
    store.store_message(&full(&attached), NOW_NS).await.unwrap();
    let file = StoredFile {
        content_type: "application/pdf".into(),
        size_bytes: 4,
        sha256: "00".into(),
    };
    store
        .mark_attachment_stored(2, 5, &file, NOW_NS)
        .await
        .unwrap();
    store.mark_attachment_failed(2, 6, NOW_NS).await.unwrap();
    for message_id in [3, 4] {
        store
            .store_list_entry(&listed(&message(message_id)), NOW_NS)
            .await
            .unwrap();
    }
    record_read_failures(&store, 4, &permanent(), 3).await;

    let counts = store.counts().await.unwrap();

    let expected = Counts {
        disclosures: 4,
        disclosures_unread: 1,
        disclosures_failed: 1,
        attachments_pending: 1,
        attachments_stored: 1,
        attachments_failed: 1,
    };
    assert_eq!(counts, expected);
}

// Pages of the failure log.

/// A store whose failure log holds, in order: a read failure of 1, one of 2,
/// a download failure of attachment 9 of 1, and a failure to list a day.
async fn store_with_failures() -> Store {
    let store = store_with_attachment().await;
    store
        .store_list_entry(&listed(&message(2)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 1, &transient(), 1).await;
    record_read_failures(&store, 2, &transient(), 1).await;
    record_attachment_failures(&store, 1, 9, &transient(), 1).await;
    store
        .record_list_failure(Date::constant(2026, 9, 22), "HTTP 503", NOW_NS)
        .await
        .unwrap();
    store
}

fn ids(failures: &[FailureRecord]) -> Vec<i64> {
    failures.iter().map(|failure| failure.id).collect()
}

#[tokio::test]
async fn failures_after__message_id__returns_only_the_failures_of_that_disclosure() {
    let store = store_with_failures().await;

    let failures = store.failures_after(Some(1), None, 10).await.unwrap();

    assert_eq!(ids(&failures), [3, 1]);
}

#[tokio::test]
async fn failures_after__after_id__continues_with_older_rows() {
    let store = store_with_failures().await;

    let failures = store.failures_after(None, Some(3), 10).await.unwrap();

    assert_eq!(ids(&failures), [2, 1]);
}

#[tokio::test]
async fn store_message__after_two_permanent_failures__gives_a_later_reread_the_full_count_again() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    let not_found = Failure {
        permanent: true,
        detail: "the exchange answered 404".into(),
    };
    store
        .record_read_failure(1, &not_found, NOW_NS)
        .await
        .unwrap();
    store
        .record_read_failure(1, &not_found, NOW_NS)
        .await
        .unwrap();
    store
        .store_message(&full(&message(1)), LATER_NS)
        .await
        .unwrap();
    let changed = with(
        message(1),
        "title",
        json!("Second quarter 2026 results (updated)"),
    );
    store
        .store_list_entry(&listed(&changed), LATER_NS)
        .await
        .unwrap();

    store
        .record_read_failure(1, &not_found, LATER_NS)
        .await
        .unwrap();

    let far_future_ns = LATER_NS + 365 * 24 * 3_600_000_000_000;
    let queue = store.unread_disclosures(far_future_ns, 10).await.unwrap();
    assert_eq!(queue, [1]);
}

// The return values of the failure records.

#[tokio::test]
async fn record_read_failure__first_transient_failure__returns_a_retry_one_minute_later() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();

    let retry = store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(
        retry,
        Retry::At {
            at_ns: ONE_MINUTE_AFTER_NOW_NS
        }
    );
}

#[tokio::test]
async fn record_read_failure__third_permanent_failure__returns_never() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 1, &permanent(), 2).await;

    let retry = store
        .record_read_failure(1, &permanent(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(retry, Retry::Never);
}

#[tokio::test]
async fn record_read_failure__transient_failure_of_a_failed_disclosure__returns_never() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 1, &permanent(), 3).await;

    let retry = store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(retry, Retry::Never);
}

#[tokio::test]
async fn record_attachment_failure__first_transient_failure__returns_a_retry_one_minute_later() {
    let store = store_with_attachment().await;

    let retry = store
        .record_attachment_failure(1, 9, &transient(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(
        retry,
        Retry::At {
            at_ns: ONE_MINUTE_AFTER_NOW_NS
        }
    );
}

#[tokio::test]
async fn record_attachment_failure__third_permanent_failure__returns_never() {
    let store = store_with_attachment().await;
    record_attachment_failures(&store, 1, 9, &permanent(), 2).await;

    let retry = store
        .record_attachment_failure(1, 9, &permanent(), NOW_NS)
        .await
        .unwrap();

    assert_eq!(retry, Retry::Never);
}

// The sizes of the queues.

#[tokio::test]
async fn queue_sizes__empty_store__counts_nothing() {
    let store = store().await;

    let sizes = store.queue_sizes().await.unwrap();

    assert_eq!(sizes, QueueSizes::default());
}

#[tokio::test]
async fn queue_sizes__listed_disclosure_waiting_for_its_retry__counts_it_unread() {
    let store = store().await;
    store
        .store_list_entry(&listed(&message(1)), NOW_NS)
        .await
        .unwrap();
    store
        .record_read_failure(1, &transient(), NOW_NS)
        .await
        .unwrap();

    let sizes = store.queue_sizes().await.unwrap();

    assert_eq!(
        sizes,
        QueueSizes {
            unread_disclosures: 1,
            ..QueueSizes::default()
        }
    );
}

#[tokio::test]
async fn queue_sizes__disclosure_read_in_full_with_an_attachment__counts_one_pending_attachment() {
    let store = store_with_attachment().await;

    let sizes = store.queue_sizes().await.unwrap();

    assert_eq!(
        sizes,
        QueueSizes {
            pending_attachments: 1,
            ..QueueSizes::default()
        }
    );
}

#[tokio::test]
async fn queue_sizes__failed_disclosure_and_failed_attachment__counts_each_failed_only() {
    let store = store_with_attachment().await;
    store
        .store_list_entry(&listed(&message(2)), NOW_NS)
        .await
        .unwrap();
    record_read_failures(&store, 2, &permanent(), 3).await;
    record_attachment_failures(&store, 1, 9, &permanent(), 3).await;

    let sizes = store.queue_sizes().await.unwrap();

    assert_eq!(
        sizes,
        QueueSizes {
            failed_disclosures: 1,
            failed_attachments: 1,
            ..QueueSizes::default()
        }
    );
}

// The delivery positions of the consumers of the event log.

#[tokio::test]
async fn delivery_position__nothing_stored__returns_none() {
    let store = store().await;

    let position = store.delivery_position("webhook").await.unwrap();

    assert_eq!(position, None);
}

#[tokio::test]
async fn set_delivery_position__new_name__is_returned_by_delivery_position() {
    let store = store().await;

    store.set_delivery_position("webhook", 7).await.unwrap();

    assert_eq!(store.delivery_position("webhook").await.unwrap(), Some(7));
}

#[tokio::test]
async fn set_delivery_position__name_already_stored__replaces_its_seq() {
    let store = store().await;
    store.set_delivery_position("webhook", 7).await.unwrap();

    store.set_delivery_position("webhook", 9).await.unwrap();

    assert_eq!(store.delivery_position("webhook").await.unwrap(), Some(9));
}

#[tokio::test]
async fn set_delivery_position__other_name__leaves_the_first_unchanged() {
    let store = store().await;
    store.set_delivery_position("webhook", 7).await.unwrap();

    store.set_delivery_position("archive", 3).await.unwrap();

    assert_eq!(store.delivery_position("webhook").await.unwrap(), Some(7));
}

// A database file, since concurrent connections need one: the in-memory store
// has a single connection.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn store_message__while_other_jobs_write__never_fails_with_database_is_locked() {
    let folder = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(
        Store::open(&folder.path().join("mirror.sqlite"))
            .await
            .unwrap(),
    );
    const WRITERS: i64 = 4;
    const WRITES: i64 = 25;

    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let store = store.clone();
            tokio::spawn(async move {
                for write in 0..WRITES {
                    let message = message(1 + writer * WRITES + write);
                    store.store_list_entry(&listed(&message), 1).await?;
                    store.store_message(&full(&message), 2).await?;
                }
                Ok::<(), xosl_newsweb::store::StoreError>(())
            })
        })
        .collect();

    for writer in writers {
        writer.await.unwrap().unwrap();
    }
}
