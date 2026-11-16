//! The gRPC server: the calls of `NewswebService`, answered from the store,
//! the files and the record of the jobs.
//!
//! A streaming call runs in a task of its own that reads the store a page at
//! a time and sends into a bounded channel; the task ends when the client
//! cancels, because the channel's receiver is then dropped.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Request, Response, Status};

use crate::clock::Clock;
use crate::files::{Files, FilesError};
use crate::ingest::{self, Job, JobState, JobStates, JobStatesSnapshot};
use crate::logging;
use crate::pb::{self, attachment, newsweb_service_server::NewswebService};
use crate::store::{self, FailureRecord, FailureStage, Position, Store, StoreError};

/// Disclosures read from the store per query of ListDisclosures.
pub const DISCLOSURE_PAGE_SIZE: u32 = 500;
/// Events read from the store per query of WatchEvents.
pub const EVENT_PAGE_SIZE: u32 = 500;
/// Failures read from the store per query of ListFailures.
pub const FAILURE_PAGE_SIZE: u32 = 500;
/// The size of each message of ReadAttachment; the last may be shorter.
pub const CHUNK_SIZE: usize = 64 * 1024;
/// Messages a streaming call holds ready before the client reads them.
const STREAM_BUFFER: usize = 32;

/// The jobs in the order GetStatus reports them.
const JOBS: [Job; 4] = [Job::Poller, Job::Backfiller, Job::Reader, Job::Downloader];

/// What GetStatus reports of the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusSettings {
    pub ingest_enabled: bool,
    pub lookback_days: u32,
}

/// The implementation of `NewswebService`.
pub struct MirrorService<C> {
    store: Arc<Store>,
    files: Arc<Files>,
    states: Arc<JobStates>,
    clock: C,
    settings: StatusSettings,
}

impl<C: Clock + Send + Sync + 'static> MirrorService<C> {
    pub fn new(
        store: Arc<Store>,
        files: Arc<Files>,
        states: Arc<JobStates>,
        clock: C,
        settings: StatusSettings,
    ) -> Self {
        Self {
            store,
            files,
            states,
            clock,
            settings,
        }
    }
}

/// A failure of the store or the files while answering a call.
#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Files(#[from] FilesError),
}

/// UNAVAILABLE when the database or the object store cannot be reached,
/// INTERNAL for every other failure.
fn error_code(error: &ServiceError) -> Code {
    let unreachable = match error {
        ServiceError::Store(StoreError::Database(error)) => matches!(
            error,
            sqlx::Error::Io(_)
                | sqlx::Error::Tls(_)
                | sqlx::Error::PoolTimedOut
                | sqlx::Error::PoolClosed
                | sqlx::Error::WorkerCrashed
        ),
        // `object_store` reports I/O and transport failures as `Generic`.
        ServiceError::Files(FilesError::ObjectStore(error)) => {
            matches!(error, object_store::Error::Generic { .. })
        }
        _ => false,
    };
    if unreachable {
        Code::Unavailable
    } else {
        Code::Internal
    }
}

/// The status sent for a failure. The message is generic; the cause stays in
/// the log.
fn error_status(code: Code) -> Status {
    match code {
        Code::Unavailable => Status::unavailable("the mirror's storage is unreachable"),
        _ => Status::internal("internal error"),
    }
}

impl From<ServiceError> for Status {
    fn from(error: ServiceError) -> Self {
        let code = error_code(&error);
        if code == Code::Internal {
            tracing::error!(error = %logging::chain(&error), "call failed");
        } else {
            tracing::warn!(error = %logging::chain(&error), "call failed");
        }
        error_status(code)
    }
}

fn store_status(error: StoreError) -> Status {
    ServiceError::from(error).into()
}

/// Checks that the disclosure holds the attachment and that its bytes are
/// stored.
fn readable_attachment(
    disclosure: Option<&pb::Disclosure>,
    message_id: i64,
    attachment_id: i64,
) -> Result<(), Status> {
    let state = disclosure
        .and_then(|disclosure| {
            disclosure
                .attachments
                .iter()
                .find(|a| a.id == attachment_id)
        })
        .map(|attachment| attachment.state());
    match state {
        None => Err(Status::not_found(format!(
            "message {message_id} has no attachment {attachment_id}"
        ))),
        Some(attachment::State::Stored) => Ok(()),
        Some(state) => Err(Status::failed_precondition(format!(
            "attachment {attachment_id} of message {message_id} is {}, not stored",
            state.as_str_name()
        ))),
    }
}

/// The file in chunks of `size` bytes, sharing its buffer; no chunk for an
/// empty file.
fn chunks(file: &Bytes, size: usize) -> Vec<Bytes> {
    (0..file.len())
        .step_by(size)
        .map(|start| file.slice(start..(start + size).min(file.len())))
        .collect()
}

/// OUT_OF_RANGE when the consumer claims an event the log does not hold yet.
fn check_since_seq(since_seq: i64, latest_seq: i64) -> Result<(), Status> {
    if since_seq > latest_seq {
        return Err(Status::out_of_range(format!(
            "since_seq {since_seq} is after the latest event, {latest_seq}"
        )));
    }
    Ok(())
}

fn job_status(name: &str, state: &JobState) -> pb::JobStatus {
    pb::JobStatus {
        name: name.to_owned(),
        last_pass_ended_at_ns: state.last_pass_ended_ns.unwrap_or_default(),
        last_success_at_ns: state.last_success_ns.unwrap_or_default(),
        last_failure_at_ns: state.last_failure.as_ref().map_or(0, |f| f.at_ns),
        last_failure: state
            .last_failure
            .as_ref()
            .map_or_else(String::new, |f| f.detail.clone()),
    }
}

fn day_range(range: &ingest::DayRange) -> pb::DayRange {
    pb::DayRange {
        first: range.first.to_string(),
        last: range.last.to_string(),
    }
}

fn coverage_message(coverage: &ingest::Coverage, lookback_days: u32) -> pb::Coverage {
    pb::Coverage {
        first_day: coverage.first_day.to_string(),
        lookback_days: i32::try_from(lookback_days).unwrap_or(i32::MAX),
        sealed: coverage.sealed.iter().map(day_range).collect(),
        gaps: coverage.gaps.iter().map(day_range).collect(),
        gap_days: i32::try_from(coverage.gap_days()).unwrap_or(i32::MAX),
    }
}

fn counts_message(counts: &store::Counts) -> pb::Counts {
    pb::Counts {
        disclosures: counts.disclosures,
        disclosures_unread: counts.disclosures_unread,
        disclosures_failed: counts.disclosures_failed,
        attachments_pending: counts.attachments_pending,
        attachments_stored: counts.attachments_stored,
        attachments_failed: counts.attachments_failed,
    }
}

/// What GetStatus reads before it builds its answer.
struct StatusParts<'a> {
    settings: StatusSettings,
    jobs: &'a JobStatesSnapshot,
    coverage: &'a ingest::Coverage,
    counts: &'a store::Counts,
    latest_seq: i64,
}

/// The answer of GetStatus. Jobs are listed only while ingest is enabled.
fn status_message(parts: &StatusParts<'_>) -> pb::Status {
    let jobs = if parts.settings.ingest_enabled {
        JOBS.iter()
            .map(|job| job_status(job.name(), parts.jobs.job(*job)))
            .collect()
    } else {
        Vec::new()
    };
    pb::Status {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        ingest_enabled: parts.settings.ingest_enabled,
        jobs,
        coverage: Some(coverage_message(
            parts.coverage,
            parts.settings.lookback_days,
        )),
        counts: Some(counts_message(parts.counts)),
        latest_seq: parts.latest_seq,
    }
}

fn failure_message(record: &FailureRecord) -> pb::Failure {
    let stage = match record.stage {
        FailureStage::List => pb::failure::Stage::List,
        FailureStage::Read => pb::failure::Stage::Read,
        FailureStage::Attachment => pb::failure::Stage::Attachment,
    };
    pb::Failure {
        id: record.id,
        at_ns: record.at_ns,
        stage: stage as i32,
        permanent: record.permanent,
        message_id: record.message_id,
        attachment_id: record.attachment_id,
        day: record.day.map_or_else(String::new, |day| day.to_string()),
        detail: record.detail.clone(),
    }
}

type Sender<T> = mpsc::Sender<Result<T, Status>>;
type Stream<T> = ReceiverStream<Result<T, Status>>;

/// Runs `produce` in a task of its own and streams what it sends.
fn spawn_stream<T, F>(produce: impl FnOnce(Sender<T>) -> F) -> Stream<T>
where
    T: Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel(STREAM_BUFFER);
    tokio::spawn(produce(sender));
    ReceiverStream::new(receiver)
}

/// Sends each item; false once the client has gone.
async fn send_all<T, U>(sender: &Sender<U>, items: &[T], message: impl Fn(&T) -> U) -> bool {
    for item in items {
        if sender.send(Ok(message(item))).await.is_err() {
            return false;
        }
    }
    true
}

async fn stream_disclosures(
    store: Arc<Store>,
    filter: pb::DisclosureFilter,
    sender: Sender<pb::ListDisclosuresResponse>,
) {
    let mut after = None;
    loop {
        let page = match store
            .disclosures(&filter, after, DISCLOSURE_PAGE_SIZE)
            .await
        {
            Ok(page) => page,
            Err(error) => {
                let _ = sender.send(Err(store_status(error))).await;
                return;
            }
        };
        // A page is short when rows that do not decode are left out, so only
        // an empty page ends the list.
        let Some(last) = page.last() else { return };
        after = Some(Position::of(last));
        let message = |disclosure: &pb::Disclosure| pb::ListDisclosuresResponse {
            disclosure: Some(disclosure.clone()),
        };
        if !send_all(&sender, &page, message).await {
            return;
        }
    }
}

/// Sends the events after `since_seq`, then each new one. `latest` was
/// subscribed before the first read, so an event committed at any time is
/// either in a read or announced afterwards; `last` keeps each event to one
/// send.
async fn stream_events(
    store: Arc<Store>,
    since_seq: i64,
    mut latest: watch::Receiver<i64>,
    sender: Sender<pb::WatchEventsResponse>,
) {
    let mut last = since_seq;
    loop {
        latest.borrow_and_update();
        loop {
            let page = match store.events_after(last, EVENT_PAGE_SIZE).await {
                Ok(page) => page,
                Err(error) => {
                    let _ = sender.send(Err(store_status(error))).await;
                    return;
                }
            };
            let Some(newest) = page.last() else { break };
            last = newest.seq;
            let message = |event: &pb::Event| pb::WatchEventsResponse {
                event: Some(event.clone()),
            };
            if !send_all(&sender, &page, message).await {
                return;
            }
        }
        tokio::select! {
            changed = latest.changed() => if changed.is_err() { return },
            () = sender.closed() => return,
        }
    }
}

async fn stream_failures(
    store: Arc<Store>,
    message_id: Option<i64>,
    sender: Sender<pb::ListFailuresResponse>,
) {
    let mut after = None;
    loop {
        let page = match store
            .failures_after(message_id, after, FAILURE_PAGE_SIZE)
            .await
        {
            Ok(page) => page,
            Err(error) => {
                let _ = sender.send(Err(store_status(error))).await;
                return;
            }
        };
        let Some(oldest) = page.last() else { return };
        after = Some(oldest.id);
        let message = |record: &FailureRecord| pb::ListFailuresResponse {
            failure: Some(failure_message(record)),
        };
        if !send_all(&sender, &page, message).await {
            return;
        }
    }
}

#[tonic::async_trait]
impl<C: Clock + Send + Sync + 'static> NewswebService for MirrorService<C> {
    type ListDisclosuresStream = Stream<pb::ListDisclosuresResponse>;
    type ReadAttachmentStream =
        tokio_stream::Iter<std::vec::IntoIter<Result<pb::ReadAttachmentResponse, Status>>>;
    type WatchEventsStream = Stream<pb::WatchEventsResponse>;
    type ListFailuresStream = Stream<pb::ListFailuresResponse>;

    async fn list_disclosures(
        &self,
        request: Request<pb::ListDisclosuresRequest>,
    ) -> Result<Response<Self::ListDisclosuresStream>, Status> {
        let filter = request.into_inner().filter.unwrap_or_default();
        let store = Arc::clone(&self.store);
        let stream = spawn_stream(|sender| stream_disclosures(store, filter, sender));
        Ok(Response::new(stream))
    }

    async fn get_disclosure(
        &self,
        request: Request<pb::GetDisclosureRequest>,
    ) -> Result<Response<pb::GetDisclosureResponse>, Status> {
        let message_id = request.into_inner().message_id;
        let disclosure = self
            .store
            .disclosure(message_id)
            .await
            .map_err(store_status)?
            .ok_or_else(|| Status::not_found(format!("no disclosure {message_id} is stored")))?;
        Ok(Response::new(pb::GetDisclosureResponse {
            disclosure: Some(disclosure),
        }))
    }

    async fn read_attachment(
        &self,
        request: Request<pb::ReadAttachmentRequest>,
    ) -> Result<Response<Self::ReadAttachmentStream>, Status> {
        let pb::ReadAttachmentRequest {
            message_id,
            attachment_id,
        } = request.into_inner();
        let disclosure = self
            .store
            .disclosure(message_id)
            .await
            .map_err(store_status)?;
        readable_attachment(disclosure.as_ref(), message_id, attachment_id)?;
        let file = self
            .files
            .get(message_id, attachment_id)
            .await
            .map_err(|error| Status::from(ServiceError::from(error)))?;
        let messages: Vec<_> = chunks(&file, CHUNK_SIZE)
            .into_iter()
            .map(|chunk| Ok(pb::ReadAttachmentResponse { chunk }))
            .collect();
        Ok(Response::new(tokio_stream::iter(messages)))
    }

    async fn watch_events(
        &self,
        request: Request<pb::WatchEventsRequest>,
    ) -> Result<Response<Self::WatchEventsStream>, Status> {
        let since_seq = request.into_inner().since_seq;
        let latest = self.store.watch_latest_seq();
        let latest_seq = self.store.latest_seq().await.map_err(store_status)?;
        check_since_seq(since_seq, latest_seq)?;
        let store = Arc::clone(&self.store);
        let stream = spawn_stream(|sender| stream_events(store, since_seq, latest, sender));
        Ok(Response::new(stream))
    }

    async fn get_status(
        &self,
        _request: Request<pb::GetStatusRequest>,
    ) -> Result<Response<pb::GetStatusResponse>, Status> {
        let now_ns = self.clock.now_ns();
        let lookback_days = self.settings.lookback_days;
        let range = ingest::lookback_range(now_ns, lookback_days);
        let listed = self
            .store
            .listed_days(range.first, range.last)
            .await
            .map_err(store_status)?;
        let counts = self.store.counts().await.map_err(store_status)?;
        let latest_seq = self.store.latest_seq().await.map_err(store_status)?;
        let jobs = self.states.snapshot();
        let coverage = ingest::coverage(&listed, now_ns, lookback_days);
        let status = status_message(&StatusParts {
            settings: self.settings,
            jobs: &jobs,
            coverage: &coverage,
            counts: &counts,
            latest_seq,
        });
        Ok(Response::new(pb::GetStatusResponse {
            status: Some(status),
        }))
    }

    async fn list_failures(
        &self,
        request: Request<pb::ListFailuresRequest>,
    ) -> Result<Response<Self::ListFailuresStream>, Status> {
        let message_id = Some(request.into_inner().message_id).filter(|id| *id != 0);
        let store = Arc::clone(&self.store);
        let stream = spawn_stream(|sender| stream_failures(store, message_id, sender));
        Ok(Response::new(stream))
    }
}

// The unit tests of the pure functions. Every call against a store is tested
// in tests/server.rs.
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::ingest::{Coverage, DayRange, JobFailure};
    use jiff::civil::Date;

    fn disclosure_with(state: attachment::State) -> pb::Disclosure {
        pb::Disclosure {
            message_id: 1,
            attachments: vec![pb::Attachment {
                id: 9,
                state: state as i32,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn code_of(result: Result<(), Status>) -> Code {
        result.map_or_else(|status| status.code(), |()| Code::Ok)
    }

    #[test]
    fn readable_attachment__states__allows_only_stored() {
        let cases = [
            (attachment::State::Stored, Code::Ok),
            (attachment::State::Pending, Code::FailedPrecondition),
            (attachment::State::Failed, Code::FailedPrecondition),
            (attachment::State::Unspecified, Code::FailedPrecondition),
        ];
        for (state, expected) in cases {
            let disclosure = disclosure_with(state);
            assert_eq!(
                code_of(readable_attachment(Some(&disclosure), 1, 9)),
                expected,
                "state: {state:?}"
            );
        }
    }

    #[test]
    fn readable_attachment__unknown_attachment__is_not_found() {
        let disclosure = disclosure_with(attachment::State::Stored);

        let result = readable_attachment(Some(&disclosure), 1, 10);

        assert_eq!(code_of(result), Code::NotFound);
    }

    #[test]
    fn readable_attachment__no_disclosure__is_not_found() {
        let result = readable_attachment(None, 1, 9);

        assert_eq!(code_of(result), Code::NotFound);
    }

    #[test]
    fn chunks__lengths__splits_at_the_chunk_size() {
        let cases: [(usize, Vec<usize>); 5] = [
            (0, vec![]),
            (1, vec![1]),
            (4, vec![4]),
            (5, vec![4, 1]),
            (12, vec![4, 4, 4]),
        ];
        for (length, expected) in cases {
            let file = Bytes::from(vec![7u8; length]);
            let lengths: Vec<usize> = chunks(&file, 4).iter().map(Bytes::len).collect();
            assert_eq!(lengths, expected, "length: {length}");
        }
    }

    #[test]
    fn chunks__joined__equal_the_file() {
        let file = Bytes::from_static(b"0123456789");

        let joined = chunks(&file, 3).concat();

        assert_eq!(joined, file);
    }

    #[test]
    fn check_since_seq__seqs__out_of_range_only_beyond_the_latest() {
        let cases = [
            (0, 0, Code::Ok),
            (3, 5, Code::Ok),
            (5, 5, Code::Ok),
            (6, 5, Code::OutOfRange),
            (1, 0, Code::OutOfRange),
        ];
        for (since_seq, latest_seq, expected) in cases {
            assert_eq!(
                code_of(check_since_seq(since_seq, latest_seq)),
                expected,
                "since: {since_seq}, latest: {latest_seq}"
            );
        }
    }

    fn generic_object_store_error() -> object_store::Error {
        object_store::Error::Generic {
            store: "LocalFileSystem",
            source: Box::new(std::io::Error::other("disk gone")),
        }
    }

    #[test]
    fn error_code__failures__unavailable_only_when_storage_is_unreachable() {
        let cases = [
            (
                StoreError::Database(sqlx::Error::PoolTimedOut).into(),
                Code::Unavailable,
            ),
            (
                StoreError::Database(sqlx::Error::PoolClosed).into(),
                Code::Unavailable,
            ),
            (
                StoreError::Database(sqlx::Error::Io(std::io::Error::other("gone"))).into(),
                Code::Unavailable,
            ),
            (
                StoreError::Database(sqlx::Error::RowNotFound).into(),
                Code::Internal,
            ),
            (
                StoreError::NoDisclosure { message_id: 1 }.into(),
                Code::Internal,
            ),
            (
                FilesError::ObjectStore(generic_object_store_error()).into(),
                Code::Unavailable,
            ),
            (
                FilesError::NotFound {
                    message_id: 1,
                    attachment_id: 9,
                }
                .into(),
                Code::Internal,
            ),
        ];
        for (error, expected) in cases {
            let error: ServiceError = error;
            assert_eq!(error_code(&error), expected, "error: {error:?}");
        }
    }

    #[test]
    fn error_status__internal__carries_a_generic_message() {
        let status = error_status(Code::Internal);

        assert_eq!(status.message(), "internal error");
    }

    fn day(day: i8) -> Date {
        Date::constant(2026, 9, day)
    }

    fn parts_with<'a>(
        ingest_enabled: bool,
        jobs: &'a JobStatesSnapshot,
        coverage: &'a Coverage,
        counts: &'a store::Counts,
    ) -> StatusParts<'a> {
        StatusParts {
            settings: StatusSettings {
                ingest_enabled,
                lookback_days: 3,
            },
            jobs,
            coverage,
            counts,
            latest_seq: 12,
        }
    }

    fn coverage() -> Coverage {
        Coverage {
            first_day: day(19),
            sealed: vec![DayRange {
                first: day(20),
                last: day(21),
            }],
            gaps: vec![DayRange {
                first: day(19),
                last: day(19),
            }],
        }
    }

    #[test]
    fn status_message__ingest_disabled__lists_no_jobs() {
        let (jobs, coverage, counts) =
            (JobStatesSnapshot::default(), coverage(), Default::default());

        let status = status_message(&parts_with(false, &jobs, &coverage, &counts));

        assert_eq!(status.jobs, []);
    }

    #[test]
    fn status_message__ingest_enabled__lists_the_jobs_in_order() {
        let (jobs, coverage, counts) =
            (JobStatesSnapshot::default(), coverage(), Default::default());

        let status = status_message(&parts_with(true, &jobs, &coverage, &counts));

        let names: Vec<&str> = status.jobs.iter().map(|job| job.name.as_str()).collect();
        assert_eq!(names, ["poller", "backfiller", "reader", "downloader"]);
    }

    #[test]
    fn status_message__coverage__writes_days_as_dates_and_counts_gap_days() {
        let (jobs, coverage, counts) =
            (JobStatesSnapshot::default(), coverage(), Default::default());

        let status = status_message(&parts_with(true, &jobs, &coverage, &counts));

        let expected = pb::Coverage {
            first_day: "2026-09-19".into(),
            lookback_days: 3,
            sealed: vec![pb::DayRange {
                first: "2026-09-20".into(),
                last: "2026-09-21".into(),
            }],
            gaps: vec![pb::DayRange {
                first: "2026-09-19".into(),
                last: "2026-09-19".into(),
            }],
            gap_days: 1,
        };
        assert_eq!(status.coverage, Some(expected));
    }

    #[test]
    fn job_status__failed_job__carries_times_and_failure() {
        let state = JobState {
            last_pass_ended_ns: Some(30),
            last_success_ns: None,
            last_failure: Some(JobFailure {
                at_ns: 20,
                detail: "HTTP 503".into(),
            }),
        };

        let status = job_status("reader", &state);

        let expected = pb::JobStatus {
            name: "reader".into(),
            last_pass_ended_at_ns: 30,
            last_success_at_ns: 0,
            last_failure_at_ns: 20,
            last_failure: "HTTP 503".into(),
        };
        assert_eq!(status, expected);
    }

    #[test]
    fn failure_message__list_failure__writes_the_day() {
        let record = FailureRecord {
            id: 4,
            at_ns: 10,
            stage: FailureStage::List,
            permanent: false,
            message_id: 0,
            attachment_id: 0,
            day: Some(day(22)),
            detail: "HTTP 503".into(),
        };

        let failure = failure_message(&record);

        let expected = pb::Failure {
            id: 4,
            at_ns: 10,
            stage: pb::failure::Stage::List as i32,
            permanent: false,
            message_id: 0,
            attachment_id: 0,
            day: "2026-09-22".into(),
            detail: "HTTP 503".into(),
        };
        assert_eq!(failure, expected);
    }
}
