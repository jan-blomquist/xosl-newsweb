//! Tests that call the real exchange. Every one is ignored by default; run
//! them with `just test-live`. The gate never does.

#![allow(non_snake_case)]

use std::time::Duration;

use xosl_newsweb::exchange::client::{Client, HttpTransport};
use xosl_newsweb::exchange::oslo_day;

/// Four requests, one second apart.
#[tokio::test]
#[ignore = "calls the real exchange"]
async fn client__today__lists_reads_one_message_and_downloads_one_attachment() {
    let client = Client::new(HttpTransport::new().unwrap(), Duration::from_secs(1));
    let now_ns = jiff::Timestamp::now().as_nanosecond() as i64;

    let page = client.list(oslo_day(now_ns)).await.unwrap();
    let Some(listed) = page.entries.first().map(|entry| &entry.disclosure) else {
        println!("the exchange lists no disclosure yet today");
        return;
    };
    let disclosure = client.message(listed.message_id).await.unwrap().disclosure;
    assert!(disclosure.read_in_full);
    assert_eq!(disclosure.message_id, listed.message_id);
    println!(
        "{} disclosures today; read {} with {} attachments",
        page.entries.len(),
        disclosure.message_id,
        disclosure.attachments.len()
    );

    if let Some(attachment) = disclosure.attachments.first() {
        let bytes = client
            .attachment(disclosure.message_id, attachment.id)
            .await
            .unwrap();
        assert!(!bytes.is_empty());
        println!("attachment {}: {} bytes", attachment.id, bytes.len());
    }
}
