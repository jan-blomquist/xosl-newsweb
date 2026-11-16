//! The messages and gRPC stubs of the contract, `com.vizroy.xosl.newsweb.v1`,
//! and the exchange's JSON responses in `raw`.
//!
//! `src/gen` is written by `just gen` from the `.proto` files under
//! `contracts/` and committed. Never edit it: change the `.proto`, then
//! regenerate.

// Generated code: not every message and stub is used, and none of it is linted.
#![allow(dead_code, clippy::all)]

include!("gen/com/vizroy/xosl/newsweb/v1/com.vizroy.xosl.newsweb.v1.rs");

/// The NewsWeb API's JSON responses, `com.vizroy.xosl.newsweb.raw.v1`.
pub mod raw {
    include!("gen/com/vizroy/xosl/newsweb/raw/v1/com.vizroy.xosl.newsweb.raw.v1.rs");
}

#[cfg(test)]
mod tests {
    use super::{Attachment, Disclosure, Issuer, attachment};
    use prost::Message;

    #[test]
    #[allow(non_snake_case)]
    fn disclosure__encoded_then_decoded__equals_original() {
        let disclosure = Disclosure {
            message_id: 654_321,
            client_announcement_id: "0f8fad5b-d9cb-469f-a165-70867728950e".into(),
            issuer: Some(Issuer {
                id: 42,
                sign: "ABC".into(),
                name: "ABC ASA".into(),
            }),
            markets: vec!["XOSL".into()],
            published_at_ns: 1_762_000_000_000_000_000,
            title: "Q3 2025".into(),
            category_ids: vec![1002, 9999],
            attachments: vec![Attachment {
                id: 7,
                name: "report.pdf".into(),
                state: attachment::State::Pending as i32,
                ..Default::default()
            }],
            ..Default::default()
        };

        let decoded = Disclosure::decode(disclosure.encode_to_vec().as_slice()).unwrap();

        assert_eq!(decoded, disclosure);
    }
}
