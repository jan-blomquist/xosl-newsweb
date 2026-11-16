//! The NewsWeb API as data: its URLs, and its responses decoded into the
//! contract's messages. Pure functions; nothing here opens a connection.
//!
//! The API is undocumented. What this module relies on was measured against
//! the live service:
//!
//! - Every response is an envelope, `{"header": {...}, "data": {...}}`. The
//!   status that counts is `header."http.code"`: a failure can arrive under
//!   HTTP 200.
//! - `list` returns the disclosures of whole Oslo calendar days. It truncates
//!   a long result and sets `data.overflow`; its `page` parameter has no
//!   effect.
//! - A `list` row is a projection: it has no body and no attachments, and its
//!   instrument and regulatory flags read as zero whatever their real values.
//!   Only `message` returns them.

// Nothing calls this module until the HTTP client is added.
#![allow(dead_code)]

use jiff::Timestamp;
use jiff::civil::Date;

use crate::pb::raw;
use crate::pb::{Attachment, Disclosure, Instrument, Issuer, attachment};

/// The disclosures of one `list` response.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub disclosures: Vec<Disclosure>,
    /// The exchange truncated the result: the day holds more disclosures than
    /// `disclosures` does.
    pub overflow: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("the exchange answered {code}: {text}")]
    Status { code: i32, text: String },
    #[error("the response is not the exchange's envelope")]
    NotEnvelope(#[source] serde_json::Error),
    #[error("the exchange answered 200 without data")]
    NoData,
    #[error("the exchange sent a disclosure without a message id")]
    NoMessageId,
    #[error("message {message_id}: publication time {value:?} is not a timestamp")]
    PublishedTime { message_id: i64, value: String },
}

impl DecodeError {
    /// A permanent failure is one a retry cannot change: the exchange states
    /// that the resource does not exist. Every other failure is transient.
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::Status {
                code: 404 | 410,
                ..
            }
        )
    }
}

/// The URL that lists the disclosures of one Oslo calendar day.
pub fn list_url(api_base: &str, day: Date) -> String {
    format!(
        "{api_base}/v1/newsreader/list?fromDate={day}&toDate={day}&category=&issuer=&market=&messageTitle="
    )
}

/// The URL of one disclosure in full.
pub fn message_url(api_base: &str, message_id: i64) -> String {
    format!("{api_base}/v1/newsreader/message?messageId={message_id}")
}

/// The URL of one attachment's bytes.
pub fn attachment_url(api_base: &str, message_id: i64, attachment_id: i64) -> String {
    format!(
        "{api_base}/v1/newsreader/attachment?messageId={message_id}&attachmentId={attachment_id}"
    )
}

/// The Oslo calendar day that contains an instant, given as UTC nanoseconds.
/// `list` takes and returns Oslo days, not UTC days.
pub fn oslo_day(at_ns: i64) -> Date {
    Timestamp::from_nanosecond(i128::from(at_ns))
        .expect("every i64 nanosecond count is within the timestamp range")
        .in_tz("Europe/Oslo")
        .expect("the bundled time zone database has Oslo")
        .date()
}

/// Decodes a `list` response. The disclosures are not read in full.
pub fn decode_list(body: &[u8]) -> Result<Page, DecodeError> {
    let response: raw::ListResponse = decode_json(body)?;
    let data = successful(response.header, response.data)?;
    let disclosures = data
        .messages
        .iter()
        .map(list_entry)
        .collect::<Result<_, _>>()?;
    Ok(Page {
        disclosures,
        overflow: data.overflow,
    })
}

/// Decodes a `message` response: one disclosure, read in full.
pub fn decode_message(body: &[u8]) -> Result<Disclosure, DecodeError> {
    let response: raw::MessageResponse = decode_json(body)?;
    let data = successful(response.header, response.data)?;
    full_disclosure(&data.message.ok_or(DecodeError::NoData)?)
}

/// Removes what a stored text must not hold: NUL and the other C0 control
/// characters except newline, carriage return and tab, and the byte-order mark.
pub fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|&c| matches!(c, '\n' | '\r' | '\t') || !(c.is_control() || c == '\u{feff}'))
        .collect()
}

fn decode_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, DecodeError> {
    serde_json::from_slice(body).map_err(DecodeError::NotEnvelope)
}

/// The envelope's data when its header reports success.
fn successful<T>(header: Option<raw::Header>, data: Option<T>) -> Result<T, DecodeError> {
    let header = header.unwrap_or_default();
    match header.http_code {
        Some(code) if code != 200 => Err(DecodeError::Status {
            code,
            text: header.result_text,
        }),
        _ => data.ok_or(DecodeError::NoData),
    }
}

/// The fields `list` reports truthfully. Its instrument and flags are
/// placeholders and are left unset.
fn list_entry(message: &raw::Message) -> Result<Disclosure, DecodeError> {
    if message.message_id == 0 {
        return Err(DecodeError::NoMessageId);
    }
    Ok(Disclosure {
        message_id: message.message_id,
        news_id: message.news_id,
        client_announcement_id: message.client_announcement_id.clone(),
        issuer: Some(Issuer {
            id: message.issuer_id,
            sign: sanitize_text(message.issuer_sign.trim()),
            name: sanitize_text(message.issuer_name.trim()),
        }),
        markets: message.markets.clone(),
        published_at_ns: published_at_ns(message)?,
        title: sanitize_text(&message.title),
        category_ids: message
            .category
            .iter()
            .map(|category| category.id)
            .collect(),
        test: message.test,
        correction_for_message_id: message.correction_for_message_id,
        ..Default::default()
    })
}

fn full_disclosure(message: &raw::Message) -> Result<Disclosure, DecodeError> {
    let instrument = (message.instr_id != 0).then(|| Instrument {
        id: message.instr_id,
        name: sanitize_text(&message.instrument_name),
        full_name: sanitize_text(&message.instrument_full_name),
    });
    let attachments = message
        .attachments
        .iter()
        .map(|attachment| Attachment {
            id: attachment.id,
            name: sanitize_text(&attachment.name),
            state: attachment::State::Pending as i32,
            ..Default::default()
        })
        .collect();

    Ok(Disclosure {
        instrument,
        body: sanitize_text(&message.body),
        read_in_full: true,
        attachments,
        oam_mandatory: message.oam_mandatory != 0,
        info_required: message.info_required != 0,
        ..list_entry(message)?
    })
}

fn published_at_ns(message: &raw::Message) -> Result<i64, DecodeError> {
    message
        .published_time
        .parse::<Timestamp>()
        .ok()
        .and_then(|at| i64::try_from(at.as_nanosecond()).ok())
        .ok_or_else(|| DecodeError::PublishedTime {
            message_id: message.message_id,
            value: message.published_time.clone(),
        })
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // Invented data in the exchange's format; the repository holds no real
    // disclosure.
    const LIST_ROW: &str = r#"{
        "id": 700001, "messageId": 700001, "newsId": 650001,
        "clientAnnouncementId": "0f8fad5b-d9cb-469f-a165-70867728950e",
        "issuerId": 4242, "issuerName": " Example Shipping ASA ", "issuerSign": "EXSH",
        "instrId": 0, "instrumentName": "", "instrumentFullName": "",
        "markets": ["XOSL"],
        "publishedTime": "2026-09-22T22:16:05.250Z",
        "title": "Example Shipping ASA: Second quarter 2026 results",
        "category": [{"id": 1002, "category_no": "HALVÅRSRAPPORT", "category_en": "HALF YEAR FINANCIAL REPORT"}],
        "numbAttachments": 2, "oamMandatory": 0, "infoRequired": 0,
        "test": false, "correctionForMessageId": 699990, "correctedByMessageId": 0
    }"#;

    const MESSAGE: &str = r#"{
        "id": 700001, "messageId": 700001, "newsId": 650001,
        "clientAnnouncementId": "0f8fad5b-d9cb-469f-a165-70867728950e",
        "issuerId": 4242, "issuerName": "Example Shipping ASA", "issuerSign": "EXSH",
        "instrId": 1305676, "instrumentName": "EXSH", "instrumentFullName": "Example Shipping",
        "markets": ["XOSL"],
        "publishedTime": "2026-09-22T22:16:05.250Z",
        "title": "Example Shipping ASA: Second quarter 2026 results",
        "body": "Revenue rose.\u0000\nSee the attached report.",
        "category": [{"id": 1002, "category_no": "HALVÅRSRAPPORT", "category_en": "HALF YEAR FINANCIAL REPORT"}],
        "attachments": [{"id": 340001, "name": "Q2-2026.pdf"}, {"id": 340002, "name": "Q2-2026-presentation.pdf"}],
        "numbAttachments": 2, "oamMandatory": 1, "infoRequired": 1,
        "test": false, "correctionForMessageId": 699990, "correctedByMessageId": 0
    }"#;

    const PUBLISHED_AT_NS: i64 = 1_790_115_365_250_000_000;

    fn list_response(rows: &str, overflow: bool) -> Vec<u8> {
        format!(
            r#"{{"header": {{"http.code": 200, "result.text": "OK"}},
                "data": {{"messages": [{rows}], "overflow": {overflow}}}}}"#
        )
        .into_bytes()
    }

    fn message_response(message: &str) -> Vec<u8> {
        format!(r#"{{"header": {{"http.code": 200}}, "data": {{"message": {message}}}}}"#)
            .into_bytes()
    }

    #[test]
    fn list_url__one_day__sets_from_date_equal_to_to_date() {
        let day = Date::constant(2026, 9, 25);

        let url = list_url("https://api.example", day);

        assert!(url.starts_with("https://api.example/v1/newsreader/list?"));
        assert!(url.contains("fromDate=2026-09-25&toDate=2026-09-25"));
    }

    #[test]
    fn attachment_url__message_and_attachment__names_both_ids() {
        let url = attachment_url("https://api.example", 700001, 340001);

        assert!(url.ends_with("/v1/newsreader/attachment?messageId=700001&attachmentId=340001"));
    }

    #[test]
    fn oslo_day__late_evening_utc__returns_next_day() {
        let day = oslo_day(PUBLISHED_AT_NS);

        assert_eq!(day, Date::constant(2026, 9, 23));
    }

    #[test]
    fn oslo_day__daylight_saving_change__returns_oslo_day() {
        let half_past_midnight_utc: Timestamp = "2026-10-25T00:30:00Z".parse().unwrap();

        let day = oslo_day(half_past_midnight_utc.as_nanosecond() as i64);

        assert_eq!(day, Date::constant(2026, 10, 25));
    }

    #[test]
    fn decode_list__code_200__returns_identities_and_publication_time() {
        let page = decode_list(&list_response(LIST_ROW, false)).unwrap();

        let disclosure = &page.disclosures[0];
        assert_eq!(disclosure.message_id, 700001);
        assert_eq!(disclosure.news_id, 650001);
        assert_eq!(disclosure.published_at_ns, PUBLISHED_AT_NS);
        assert_eq!(disclosure.category_ids, [1002]);
        assert_eq!(disclosure.correction_for_message_id, 699990);
    }

    #[test]
    fn decode_list__issuer_name_padded__trims_it() {
        let page = decode_list(&list_response(LIST_ROW, false)).unwrap();

        let issuer = page.disclosures[0].issuer.as_ref().unwrap();
        assert_eq!(issuer.name, "Example Shipping ASA");
        assert_eq!(issuer.sign, "EXSH");
    }

    #[test]
    fn decode_list__row_with_zero_instrument__leaves_instrument_unset() {
        let page = decode_list(&list_response(LIST_ROW, false)).unwrap();

        let disclosure = &page.disclosures[0];
        assert_eq!(disclosure.instrument, None);
        assert!(!disclosure.read_in_full);
    }

    #[test]
    fn decode_list__overflow_set__reports_overflow() {
        let page = decode_list(&list_response(LIST_ROW, true)).unwrap();

        assert!(page.overflow);
    }

    #[test]
    fn decode_list__only_required_fields__decodes() {
        let row = r#"{"messageId": 1, "publishedTime": "2026-01-01T00:00:00Z", "title": "t"}"#;

        let page = decode_list(&list_response(row, false)).unwrap();

        assert_eq!(page.disclosures[0].message_id, 1);
    }

    #[test]
    fn decode_list__row_without_message_id__returns_error() {
        let row = r#"{"publishedTime": "2026-01-01T00:00:00Z", "title": "t"}"#;

        let error = decode_list(&list_response(row, false)).unwrap_err();

        assert!(matches!(error, DecodeError::NoMessageId));
    }

    #[test]
    fn decode_list__code_404_under_http_200__returns_permanent_error() {
        let body = br#"{"header": {"http.code": 404, "result.text": "Not found"}, "data": null}"#;

        let error = decode_list(body).unwrap_err();

        assert!(matches!(error, DecodeError::Status { code: 404, .. }));
        assert!(error.is_permanent());
    }

    #[test]
    fn decode_list__code_503__returns_transient_error() {
        let body = br#"{"header": {"http.code": 503, "result.text": "Busy"}, "data": null}"#;

        let error = decode_list(body).unwrap_err();

        assert!(!error.is_permanent());
    }

    #[test]
    fn decode_list__body_not_json__returns_transient_error() {
        let error = decode_list(b"<html>Bad gateway</html>").unwrap_err();

        assert!(matches!(error, DecodeError::NotEnvelope(_)));
        assert!(!error.is_permanent());
    }

    #[test]
    fn decode_list__publication_time_malformed__names_message_and_value() {
        let row = r#"{"messageId": 7, "publishedTime": "yesterday", "title": "t"}"#;

        let error = decode_list(&list_response(row, false)).unwrap_err();

        assert_eq!(
            error.to_string(),
            r#"message 7: publication time "yesterday" is not a timestamp"#
        );
    }

    #[test]
    fn decode_message__instrument_present__returns_instrument() {
        let disclosure = decode_message(&message_response(MESSAGE)).unwrap();

        let instrument = disclosure.instrument.unwrap();
        assert_eq!(instrument.id, 1305676);
        assert_eq!(instrument.full_name, "Example Shipping");
    }

    #[test]
    fn decode_message__full_message__is_read_in_full_with_flags() {
        let disclosure = decode_message(&message_response(MESSAGE)).unwrap();

        assert!(disclosure.read_in_full);
        assert!(disclosure.oam_mandatory);
        assert!(disclosure.info_required);
    }

    #[test]
    fn decode_message__two_attachments__returns_both_as_pending() {
        let disclosure = decode_message(&message_response(MESSAGE)).unwrap();

        let ids: Vec<i64> = disclosure.attachments.iter().map(|a| a.id).collect();
        assert_eq!(ids, [340001, 340002]);
        assert!(
            disclosure
                .attachments
                .iter()
                .all(|a| a.state == attachment::State::Pending as i32)
        );
    }

    #[test]
    fn decode_message__nul_in_body__removes_nul() {
        let disclosure = decode_message(&message_response(MESSAGE)).unwrap();

        assert_eq!(disclosure.body, "Revenue rose.\nSee the attached report.");
    }

    #[test]
    fn sanitize_text__control_characters_and_byte_order_mark__keeps_only_whitespace_controls() {
        let cleaned = sanitize_text("\u{feff}a\u{0}b\u{1b}c\td\r\ne");

        assert_eq!(cleaned, "abc\td\r\ne");
    }
}
