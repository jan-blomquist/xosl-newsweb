//! Integration tests of the store: they run the schema and every query
//! against SQLite itself. All but one use a database in memory; the last one
//! writes a file in a temporary folder.

#![allow(non_snake_case)]

use serde_json::{Value, json};
use xosl_newsweb::exchange::{self, Fetched};
use xosl_newsweb::pb::{DisclosureFilter, attachment, event};
use xosl_newsweb::store::{Position, Store, StoredFile};

const NOON_NS: i64 = 1_790_078_400_000_000_000;
const NOW_NS: i64 = NOON_NS + 60_000_000_000;
const LATER_NS: i64 = NOW_NS + 60_000_000_000;

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

async fn store() -> Store {
    Store::open_in_memory().await.unwrap()
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
