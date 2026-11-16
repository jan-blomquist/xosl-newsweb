//! The webhook: each event of the log, sent to a URL as an HTTP POST, in
//! order.
//!
//! Delivery is at least once: the `seq` of the last delivered event is
//! stored after the receiver accepted it, so an event whose position was not
//! yet stored is sent again after a restart. An event is sent only after the
//! one before it was accepted, and a failed delivery is tried again until it
//! succeeds.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use tokio_util::sync::CancellationToken;

use crate::logging;
use crate::pb;
use crate::store::{Store, StoreError};

/// The longest delay between two tries of a failed delivery.
pub const MAX_RETRY_DELAY: Duration = Duration::from_secs(5 * 60);

/// Consecutive failures of one event after which an error is logged.
pub const FAILURES_BEFORE_ERROR: u32 = 5;

/// The header that holds the event's `seq`.
pub const SEQ_HEADER: &str = "X-Xosl-Newsweb-Seq";

/// The header that holds `sha256=<hex>`, the HMAC-SHA256 of the body keyed by
/// the secret.
pub const SIGNATURE_HEADER: &str = "X-Xosl-Newsweb-Signature";

/// The name of the webhook's row among the stored delivery positions.
pub const POSITION_NAME: &str = "webhook";

/// The time limit of one HTTP request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Events read from the store at a time.
const EVENT_PAGE_SIZE: u32 = 100;

const USER_AGENT: &str = concat!("xosl-newsweb/", env!("CARGO_PKG_VERSION"));

/// Where and how the events are delivered.
#[derive(Debug, Clone)]
pub struct WebhookSettings {
    pub url: String,
    /// Signs each request when set.
    pub secret: Option<String>,
    /// The delay before the second try of a failed delivery; it doubles up to `MAX_RETRY_DELAY`.
    pub first_retry_delay: Duration,
}

/// One HTTP POST of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub url: String,
    /// Header names and values, in the order they are sent.
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// A POST that did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum PostError {
    #[error("the request failed")]
    Request(#[source] reqwest::Error),
    #[error("HTTP {0}")]
    Status(u16),
}

/// A delivery that could not be made or was not accepted.
#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    #[error("cannot encode the event as JSON")]
    Encode(#[source] serde_json::Error),
    #[error("cannot key HMAC-SHA256 with the secret")]
    Key,
    #[error(transparent)]
    Post(#[from] PostError),
}

/// One HTTP POST. The seam between the delivery logic and the network.
pub trait Post {
    /// Sends the request; `Ok` when the receiver answered with a 2xx status.
    fn post(&self, request: &Delivery) -> impl Future<Output = Result<(), PostError>> + Send;
}

/// The POST over HTTP(S), with a time limit of `REQUEST_TIMEOUT`.
pub struct HttpPost {
    http: reqwest::Client,
}

impl HttpPost {
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self { http })
    }
}

impl Post for HttpPost {
    async fn post(&self, request: &Delivery) -> Result<(), PostError> {
        let mut builder = self.http.post(&request.url).body(request.body.clone());
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        let response = builder.send().await.map_err(PostError::Request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(PostError::Status(status.as_u16()));
        }
        Ok(())
    }
}

/// The scheme, host and port of a webhook URL, for the log. The path and the
/// query are left out: webhook URLs often carry a token in them.
pub fn receiver(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "an unparsable URL".to_owned())
}

/// The request that delivers `event` to `url`: the event's proto-JSON as the
/// body, its `seq` in `SEQ_HEADER`, and, with a secret, the body's signature
/// in `SIGNATURE_HEADER`.
pub fn delivery(
    url: &str,
    event: &pb::Event,
    secret: Option<&str>,
) -> Result<Delivery, DeliveryError> {
    let body = serde_json::to_vec(event).map_err(DeliveryError::Encode)?;
    let mut headers = vec![
        ("Content-Type", "application/json".to_owned()),
        (SEQ_HEADER, event.seq.to_string()),
    ];
    if let Some(secret) = secret {
        headers.push((SIGNATURE_HEADER, signature(secret.as_bytes(), &body)?));
    }
    Ok(Delivery {
        url: url.to_owned(),
        headers,
        body,
    })
}

/// `sha256=` and the lowercase hex HMAC-SHA256 of `body`, keyed by `key`.
pub fn signature(key: &[u8], body: &[u8]) -> Result<String, DeliveryError> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).map_err(|_| DeliveryError::Key)?;
    mac.update(body);
    Ok(format!(
        "sha256={}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

/// The delay before the next try after `consecutive_failures` failures in a
/// row: `first` after the first, doubling with each further failure, at most
/// `MAX_RETRY_DELAY`. Zero when nothing has failed.
pub fn retry_delay(first: Duration, consecutive_failures: u32) -> Duration {
    if consecutive_failures == 0 {
        return Duration::ZERO;
    }
    let factor = 2u32.saturating_pow(consecutive_failures - 1);
    first.saturating_mul(factor).min(MAX_RETRY_DELAY)
}

/// Delivers the events of the log to the webhook until `shutdown` is cancelled.
///
/// Without a stored position, delivery starts after the newest event in the
/// log. A store error is logged and the read is tried again after the retry
/// delay.
pub async fn run<P: Post>(
    store: Arc<Store>,
    post: P,
    settings: WebhookSettings,
    shutdown: &CancellationToken,
) {
    let mut latest = store.watch_latest_seq();
    let first = settings.first_retry_delay;
    let Some(mut position) = retry_store(first, shutdown, "read the delivery position", || {
        start_position(&store)
    })
    .await
    else {
        return;
    };
    tracing::info!(
        receiver = receiver(&settings.url),
        after_seq = position,
        "delivering events to the webhook"
    );

    loop {
        // Marked as seen before the read: an event committed after this line
        // is either in the page or changes `latest` again.
        latest.borrow_and_update();
        let Some(events) = retry_store(first, shutdown, "read the event log", || {
            store.events_after(position, EVENT_PAGE_SIZE)
        })
        .await
        else {
            return;
        };
        if events.is_empty() {
            tokio::select! {
                changed = latest.changed() => if changed.is_err() { return },
                () = shutdown.cancelled() => return,
            }
            continue;
        }
        for event in &events {
            if !deliver(&post, &settings, event, shutdown).await {
                return;
            }
            position = event.seq;
            // The position held here stays correct; a later success writes it
            // again, and until then a restart sends some events twice.
            if let Err(error) = store.set_delivery_position(POSITION_NAME, position).await {
                tracing::error!(
                    seq = position,
                    error = %logging::chain(&error),
                    "cannot store the webhook's delivery position",
                );
            }
        }
    }
}

/// The stored position, or, when none is stored, the newest `seq` of the log,
/// which is then stored.
async fn start_position(store: &Store) -> Result<i64, StoreError> {
    if let Some(seq) = store.delivery_position(POSITION_NAME).await? {
        return Ok(seq);
    }
    let seq = store.latest_seq().await?;
    store.set_delivery_position(POSITION_NAME, seq).await?;
    Ok(seq)
}

/// Calls `read` until it succeeds, waiting the retry delay after each failure.
/// `None` when `shutdown` is cancelled first.
async fn retry_store<T, F, Fut>(
    first_retry_delay: Duration,
    shutdown: &CancellationToken,
    action: &str,
    mut read: F,
) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, StoreError>>,
{
    let mut failures = 0;
    loop {
        let error = tokio::select! {
            result = read() => match result {
                Ok(value) => return Some(value),
                Err(error) => error,
            },
            () = shutdown.cancelled() => return None,
        };
        failures += 1;
        let delay = retry_delay(first_retry_delay, failures);
        tracing::error!(
            error = %logging::chain(&error),
            delay_ms = delay.as_millis() as u64,
            "webhook: cannot {action}; trying again",
        );
        if !wait(delay, shutdown).await {
            return None;
        }
    }
}

/// Sends `event` until a try succeeds. `false` when `shutdown` is cancelled
/// first.
async fn deliver<P: Post>(
    post: &P,
    settings: &WebhookSettings,
    event: &pb::Event,
    shutdown: &CancellationToken,
) -> bool {
    let seq = event.seq;
    let mut failures = 0;
    loop {
        let result = match delivery(&settings.url, event, settings.secret.as_deref()) {
            Ok(request) => tokio::select! {
                result = post.post(&request) => result.map_err(DeliveryError::Post),
                () = shutdown.cancelled() => return false,
            },
            Err(error) => Err(error),
        };
        let error = match result {
            Ok(()) if failures == 0 => {
                tracing::debug!(seq, "delivered to the webhook");
                return true;
            }
            Ok(()) => {
                tracing::info!(seq, failures, "delivered to the webhook after failures");
                return true;
            }
            Err(error) => error,
        };
        failures += 1;
        let delay = retry_delay(settings.first_retry_delay, failures);
        tracing::warn!(
            seq,
            error = %logging::chain(&error),
            delay_ms = delay.as_millis() as u64,
            "webhook delivery failed; trying again",
        );
        if failures == FAILURES_BEFORE_ERROR {
            tracing::error!(
                seq,
                failures,
                "webhook deliveries are stopped at this event until it is delivered",
            );
        }
        if !wait(delay, shutdown).await {
            return false;
        }
    }
}

/// Waits `delay`; `false` when `shutdown` is cancelled first.
async fn wait(delay: Duration, shutdown: &CancellationToken) -> bool {
    tokio::select! {
        () = tokio::time::sleep(delay) => true,
        () = shutdown.cancelled() => false,
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    const URL: &str = "https://receiver.example/hook";
    /// RFC 4231, test case 2.
    const RFC_4231_KEY: &[u8] = b"Jefe";
    const RFC_4231_DATA: &[u8] = b"what do ya want for nothing?";
    const RFC_4231_HMAC_SHA256: &str =
        "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";

    fn event(seq: i64, message_id: i64) -> pb::Event {
        pb::Event {
            seq,
            kind: pb::event::Kind::Discovered as i32,
            at_ns: 1_790_078_460_000_000_000,
            disclosure: Some(pb::Disclosure {
                message_id,
                title: "Second quarter 2026 results".into(),
                ..pb::Disclosure::default()
            }),
            attachment_id: 0,
        }
    }

    fn header<'a>(request: &'a Delivery, name: &str) -> Option<&'a str> {
        request
            .headers
            .iter()
            .find(|(header, _)| *header == name)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn delivery__event__body_is_the_event_as_json_with_seq_and_message_id() {
        let request = delivery(URL, &event(7, 4711), None).unwrap();

        let decoded: pb::Event = serde_json::from_slice(&request.body).unwrap();
        let message_id = decoded.disclosure.map(|disclosure| disclosure.message_id);
        assert_eq!((decoded.seq, message_id), (7, Some(4711)));
    }

    #[test]
    fn receiver__url_with_a_token_in_path_and_query__returns_scheme_host_and_port_only() {
        let logged = receiver("https://hooks.example:8443/t/SECRET123?key=SECRET456");

        assert_eq!(logged, "https://hooks.example:8443");
    }

    #[test]
    fn delivery__event__sets_url_content_type_and_seq_header() {
        let request = delivery(URL, &event(7, 4711), None).unwrap();

        let actual = (
            request.url.as_str(),
            header(&request, "Content-Type"),
            header(&request, SEQ_HEADER),
        );
        assert_eq!(actual, (URL, Some("application/json"), Some("7")));
    }

    #[test]
    fn delivery__no_secret__has_no_signature_header() {
        let request = delivery(URL, &event(7, 4711), None).unwrap();

        assert_eq!(header(&request, SIGNATURE_HEADER), None);
    }

    #[test]
    fn delivery__secret__signature_header_is_the_hmac_of_the_body() {
        let request = delivery(URL, &event(7, 4711), Some("Jefe")).unwrap();
        let expected = signature(RFC_4231_KEY, &request.body).unwrap();

        assert_eq!(header(&request, SIGNATURE_HEADER), Some(expected.as_str()));
    }

    #[test]
    fn signature__rfc_4231_test_case_2__returns_the_published_value() {
        let actual = signature(RFC_4231_KEY, RFC_4231_DATA).unwrap();

        assert_eq!(actual, RFC_4231_HMAC_SHA256);
    }

    #[test]
    fn retry_delay__consecutive_failures__doubles_from_the_first_delay_up_to_the_maximum() {
        let first = Duration::from_secs(1);
        let cases = [
            (0, Duration::ZERO),
            (1, Duration::from_secs(1)),
            (2, Duration::from_secs(2)),
            (3, Duration::from_secs(4)),
            (9, Duration::from_secs(256)),
            (10, MAX_RETRY_DELAY),
            (u32::MAX, MAX_RETRY_DELAY),
        ];

        for (failures, expected) in cases {
            assert_eq!(
                retry_delay(first, failures),
                expected,
                "failures: {failures}"
            );
        }
    }
}
