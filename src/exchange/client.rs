//! The client of the NewsWeb API: one day's list, one disclosure in full, one
//! attachment's bytes.
//!
//! Every request, `urls.json` included, passes one rate limiter. A transient
//! failure is retried within the call; a permanent one is returned at once.
//! The API's base URL is read from `urls.json` on the first call and again
//! after a failure to reach the host, so the client follows the API when the
//! exchange moves it.

use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use bytes::Bytes;
use jiff::civil::Date;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::{DecodeError, Fetched, Page};

/// Retries of one call after its first try; the waits before them are 2 s
/// and 8 s.
const MAX_RETRIES: usize = 2;
const FIRST_RETRY_DELAY: Duration = Duration::from_secs(2);
const RETRY_DELAY_FACTOR: f32 = 4.0;
/// How long one try may take.
const TRY_TIMEOUT: Duration = Duration::from_secs(30);
/// The shortest time between two reads of `urls.json`.
const API_BASE_MIN_AGE: Duration = Duration::from_secs(60);
const USER_AGENT: &str = concat!("xosl-newsweb/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("{url}: {source}")]
    Transport {
        url: String,
        #[source]
        source: TransportError,
    },
    #[error("{url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: DecodeError,
    },
}

impl ClientError {
    /// A permanent failure is one a retry cannot change: the exchange states
    /// that the resource does not exist. Every other failure is transient.
    pub fn is_permanent(&self) -> bool {
        match self {
            Self::Transport { source, .. } => {
                matches!(source, TransportError::Status(404 | 410))
            }
            Self::Decode { source, .. } => source.is_permanent(),
        }
    }

    /// The failure suggests that the API is no longer at the known base URL.
    fn suggests_moved_api(&self) -> bool {
        matches!(
            self,
            Self::Transport {
                source: TransportError::Unreachable(_),
                ..
            } | Self::Decode {
                source: DecodeError::NotEnvelope(_),
                ..
            }
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// DNS, connect or TLS failed.
    #[error("cannot reach the host: {0}")]
    Unreachable(String),
    #[error("timed out")]
    Timeout,
    #[error("HTTP {0}")]
    Status(u16),
    #[error("{0}")]
    Other(String),
}

/// One HTTP GET. The seam between the client's logic and the network.
pub trait Transport {
    fn get(&self, url: &str) -> impl Future<Output = Result<Bytes, TransportError>> + Send;
}

/// The transport over HTTPS.
pub struct HttpTransport {
    http: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TRY_TIMEOUT)
            .build()?;
        Ok(Self { http })
    }
}

impl Transport for HttpTransport {
    async fn get(&self, url: &str) -> Result<Bytes, TransportError> {
        let response = self.http.get(url).send().await.map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(TransportError::Status(status.as_u16()));
        }
        response.bytes().await.map_err(transport_error)
    }
}

fn transport_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else if error.is_connect() {
        TransportError::Unreachable(error.to_string())
    } else {
        TransportError::Other(error.to_string())
    }
}

/// Spaces requests by at least `min_interval`; concurrent callers wait in turn.
struct RateLimiter {
    min_interval: Duration,
    /// The earliest instant the next request may leave.
    next: Mutex<Instant>,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            next: Mutex::new(Instant::now()),
        }
    }

    async fn wait(&self) {
        // The lock is held across the sleep, so waiting callers queue.
        let mut next = self.next.lock().await;
        tokio::time::sleep_until(*next).await;
        *next = Instant::now() + self.min_interval;
    }
}

struct ApiBase {
    url: String,
    resolved_at: Instant,
}

/// The calls of the NewsWeb API that ingest makes. The seam between the
/// ingest jobs and the client; each method has the contract of the
/// `Client` method of the same name.
pub trait Exchange {
    /// The disclosures of one Oslo calendar day, not read in full.
    fn list(&self, day: Date) -> impl Future<Output = Result<Page, ClientError>> + Send;

    /// One disclosure, read in full.
    fn message(&self, message_id: i64)
    -> impl Future<Output = Result<Fetched, ClientError>> + Send;

    /// One attachment's bytes, unchanged.
    fn attachment(
        &self,
        message_id: i64,
        attachment_id: i64,
    ) -> impl Future<Output = Result<Bytes, ClientError>> + Send;
}

pub struct Client<T = HttpTransport> {
    transport: T,
    limiter: RateLimiter,
    api_base: Mutex<Option<ApiBase>>,
}

impl<T: Transport + Sync> Exchange for Client<T> {
    fn list(&self, day: Date) -> impl Future<Output = Result<Page, ClientError>> + Send {
        Client::list(self, day)
    }

    fn message(
        &self,
        message_id: i64,
    ) -> impl Future<Output = Result<Fetched, ClientError>> + Send {
        Client::message(self, message_id)
    }

    fn attachment(
        &self,
        message_id: i64,
        attachment_id: i64,
    ) -> impl Future<Output = Result<Bytes, ClientError>> + Send {
        Client::attachment(self, message_id, attachment_id)
    }
}

impl<T: Transport> Client<T> {
    /// Sends no request: `urls.json` is read on the first call.
    pub fn new(transport: T, min_interval: Duration) -> Self {
        Self {
            transport,
            limiter: RateLimiter::new(min_interval),
            api_base: Mutex::new(None),
        }
    }

    /// The disclosures of one Oslo calendar day, not read in full.
    pub async fn list(&self, day: Date) -> Result<Page, ClientError> {
        self.call(
            |api_base| super::list_url(api_base, day),
            |body| super::decode_list(&body),
        )
        .await
    }

    /// One disclosure, read in full.
    pub async fn message(&self, message_id: i64) -> Result<Fetched, ClientError> {
        self.call(
            |api_base| super::message_url(api_base, message_id),
            |body| super::decode_message(&body),
        )
        .await
    }

    /// One attachment's bytes, unchanged.
    pub async fn attachment(
        &self,
        message_id: i64,
        attachment_id: i64,
    ) -> Result<Bytes, ClientError> {
        self.call(
            |api_base| super::attachment_url(api_base, message_id, attachment_id),
            Ok,
        )
        .await
    }

    /// One call to the API, retried while the failure is transient.
    async fn call<R>(
        &self,
        url: impl Fn(&str) -> String,
        decode: impl Fn(Bytes) -> Result<R, DecodeError>,
    ) -> Result<R, ClientError> {
        let retries = ExponentialBuilder::default()
            .with_min_delay(FIRST_RETRY_DELAY)
            .with_factor(RETRY_DELAY_FACTOR)
            .with_max_times(MAX_RETRIES);
        (|| self.try_call(&url, &decode))
            .retry(retries)
            .when(|error| !error.is_permanent())
            .await
    }

    async fn try_call<R>(
        &self,
        url: &impl Fn(&str) -> String,
        decode: &impl Fn(Bytes) -> Result<R, DecodeError>,
    ) -> Result<R, ClientError> {
        let api_base = self.api_base().await?;
        let url = url(&api_base);
        let result = match self.get(&url).await {
            Ok(body) => decode(body).map_err(|source| ClientError::Decode { url, source }),
            Err(error) => Err(error),
        };
        if let Err(error) = &result
            && error.suggests_moved_api()
        {
            self.forget_api_base().await;
        }
        result
    }

    async fn get(&self, url: &str) -> Result<Bytes, ClientError> {
        self.limiter.wait().await;
        self.transport
            .get(url)
            .await
            .map_err(|source| ClientError::Transport {
                url: url.to_owned(),
                source,
            })
    }

    /// The API's base URL, read from `urls.json` when not known.
    async fn api_base(&self) -> Result<String, ClientError> {
        let mut api_base = self.api_base.lock().await;
        if let Some(known) = api_base.as_ref() {
            return Ok(known.url.clone());
        }
        let body = self.get(super::URLS_JSON).await?;
        let url = super::decode_api_base(&body).map_err(|source| ClientError::Decode {
            url: super::URLS_JSON.to_owned(),
            source,
        })?;
        tracing::info!(api_base = url, "read the API base from urls.json");
        *api_base = Some(ApiBase {
            url: url.clone(),
            resolved_at: Instant::now(),
        });
        Ok(url)
    }

    /// Makes the next call read `urls.json` again, unless it was read within
    /// the last minute.
    async fn forget_api_base(&self) {
        let mut api_base = self.api_base.lock().await;
        if api_base
            .as_ref()
            .is_some_and(|known| known.resolved_at.elapsed() >= API_BASE_MIN_AGE)
        {
            *api_base = None;
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    use super::*;

    const INTERVAL: Duration = Duration::from_millis(500);
    const URLS: &str = r#"{"api_large": "https://api.example"}"#;
    const MOVED_URLS: &str = r#"{"api_large": "https://api2.example"}"#;
    const EMPTY_LIST: &str =
        r#"{"header": {"http.code": 200}, "data": {"messages": [], "overflow": false}}"#;
    const NOT_FOUND: &str = r#"{"header": {"http.code": 404, "result.text": "Not found"}}"#;

    /// Answers each request with the next scripted response and records the URL.
    #[derive(Default)]
    struct FakeTransport {
        responses: StdMutex<VecDeque<Result<Bytes, TransportError>>>,
        requested: StdMutex<Vec<String>>,
    }

    impl FakeTransport {
        fn answering(responses: Vec<Result<&'static str, TransportError>>) -> Arc<Self> {
            let responses = responses
                .into_iter()
                .map(|response| response.map(|body| Bytes::from_static(body.as_bytes())))
                .collect();
            Arc::new(Self {
                responses: StdMutex::new(responses),
                requested: StdMutex::default(),
            })
        }

        fn requested(&self) -> Vec<String> {
            self.requested.lock().unwrap().clone()
        }
    }

    impl Transport for Arc<FakeTransport> {
        async fn get(&self, url: &str) -> Result<Bytes, TransportError> {
            self.requested.lock().unwrap().push(url.to_owned());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("the test scripted a response for every request")
        }
    }

    fn client(transport: &Arc<FakeTransport>) -> Client<Arc<FakeTransport>> {
        Client::new(Arc::clone(transport), INTERVAL)
    }

    fn day() -> Date {
        Date::constant(2026, 9, 25)
    }

    #[tokio::test(start_paused = true)]
    async fn new__no_call_made__sends_no_request() {
        let transport = FakeTransport::answering(vec![]);

        let _client = client(&transport);

        assert!(transport.requested().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn list__first_call__reads_urls_json_then_calls_the_named_host() {
        let transport = FakeTransport::answering(vec![Ok(URLS), Ok(EMPTY_LIST)]);

        client(&transport).list(day()).await.unwrap();

        let requested = transport.requested();
        assert_eq!(requested[0], "https://newsweb.oslobors.no/urls.json");
        assert!(requested[1].starts_with("https://api.example/v1/newsreader/list?"));
    }

    #[tokio::test(start_paused = true)]
    async fn list__second_call__does_not_read_urls_json_again() {
        let transport = FakeTransport::answering(vec![Ok(URLS), Ok(EMPTY_LIST), Ok(EMPTY_LIST)]);
        let client = client(&transport);

        client.list(day()).await.unwrap();
        client.list(day()).await.unwrap();

        assert_eq!(transport.requested().len(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn list__host_unreachable_and_base_moved__calls_the_new_host() {
        let transport = FakeTransport::answering(vec![
            Ok(URLS),
            Ok(EMPTY_LIST),
            Err(TransportError::Unreachable("connection refused".into())),
            Ok(MOVED_URLS),
            Ok(EMPTY_LIST),
        ]);
        let client = client(&transport);
        client.list(day()).await.unwrap();
        tokio::time::sleep(API_BASE_MIN_AGE).await;

        client.list(day()).await.unwrap();

        let requested = transport.requested();
        assert!(requested[4].starts_with("https://api2.example/"));
    }

    #[tokio::test(start_paused = true)]
    async fn list__host_unreachable_within_a_minute_of_reading_urls_json__keeps_the_base() {
        let transport = FakeTransport::answering(vec![
            Ok(URLS),
            Err(TransportError::Unreachable("connection refused".into())),
            Ok(EMPTY_LIST),
        ]);

        client(&transport).list(day()).await.unwrap();

        let requested = transport.requested();
        assert!(requested[2].starts_with("https://api.example/"));
    }

    #[tokio::test(start_paused = true)]
    async fn message__envelope_404__returns_permanent_error_without_retry() {
        let transport = FakeTransport::answering(vec![Ok(URLS), Ok(NOT_FOUND)]);

        let error = client(&transport).message(7).await.unwrap_err();

        assert!(error.is_permanent());
        assert_eq!(transport.requested().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn list__brief_503__succeeds_on_retry() {
        let transport = FakeTransport::answering(vec![
            Ok(URLS),
            Err(TransportError::Status(503)),
            Ok(EMPTY_LIST),
        ]);

        let page = client(&transport).list(day()).await;

        assert!(page.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn list__three_timeouts__returns_transient_error() {
        let transport = FakeTransport::answering(vec![
            Ok(URLS),
            Err(TransportError::Timeout),
            Err(TransportError::Timeout),
            Err(TransportError::Timeout),
        ]);

        let error = client(&transport).list(day()).await.unwrap_err();

        assert!(!error.is_permanent());
        assert_eq!(transport.requested().len(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn attachment__pdf__returns_bytes_unchanged() {
        const PDF: &str = "%PDF-1.7\n\u{0}binary";
        let transport = FakeTransport::answering(vec![Ok(URLS), Ok(PDF)]);

        let bytes = client(&transport).attachment(7, 9).await.unwrap();

        assert_eq!(bytes, PDF.as_bytes());
    }

    #[tokio::test(start_paused = true)]
    async fn attachment__http_404__returns_permanent_error_naming_both_ids() {
        let transport = FakeTransport::answering(vec![Ok(URLS), Err(TransportError::Status(404))]);

        let error = client(&transport).attachment(7, 9).await.unwrap_err();

        assert!(error.is_permanent());
        assert!(error.to_string().contains("messageId=7&attachmentId=9"));
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limiter__three_concurrent_calls__spaces_them_one_interval_apart() {
        let limiter = Arc::new(RateLimiter::new(INTERVAL));
        let start = Instant::now();

        let waiters: Vec<_> = (0..3)
            .map(|_| {
                let limiter = Arc::clone(&limiter);
                tokio::spawn(async move { limiter.wait().await })
            })
            .collect();
        for waiter in waiters {
            waiter.await.unwrap();
        }

        assert_eq!(start.elapsed(), INTERVAL * 2);
    }
}
