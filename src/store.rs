//! The mirror's database: disclosures, the state of their attachments, and
//! the event log. SQLite, in one file.
//!
//! A disclosure is kept as the exchange's JSON for it and decoded on every
//! read, so a fix to the decoder applies to what is already stored. Every
//! change to a disclosure and the event that reports it are written in one
//! transaction.
//!
//! The store reads no clock: callers pass the current time.
//!
//! The `seq` of the newest event is also kept in a `watch` channel, set after
//! each transaction that wrote an event commits, so a reader waits for new
//! events without polling.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use jiff::civil::Date;
use prost::Message;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};
use tokio::sync::watch;

use crate::exchange::{self, DecodeError, Fetched};
use crate::pb::{Attachment, Disclosure, DisclosureFilter, Event, attachment, event};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("message {message_id}: the stored document does not decode")]
    Document {
        message_id: i64,
        #[source]
        source: DecodeError,
    },
    #[error("event {seq}: the stored disclosure does not decode")]
    EventDisclosure {
        seq: i64,
        #[source]
        source: prost::DecodeError,
    },
    #[error("message {message_id} has no attachment {attachment_id}")]
    NoAttachment { message_id: i64, attachment_id: i64 },
    #[error("no disclosure {message_id} is stored")]
    NoDisclosure { message_id: i64 },
    #[error("failure {id}: stage {stage} is not a stage")]
    FailureStage { id: i64, stage: i64 },
    #[error("the stored day {value:?} is not a date")]
    Day {
        value: String,
        #[source]
        source: jiff::Error,
    },
}

/// Permanent failures after which a disclosure or an attachment is not
/// requested again.
pub const MAX_PERMANENT_FAILURES: u32 = 3;

/// How long a write waits for SQLite's write lock while another job holds
/// it. SQLite's default of 5 seconds is too short on a slow disk, where one
/// commit can take more than a second and several jobs wait in turn.
const WRITE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How long a caller of an in-memory store waits for its one connection.
const IN_MEMORY_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(365 * 86_400);

const MINUTE_NS: i64 = 60_000_000_000;
const DAY_NS: i64 = 24 * 60 * MINUTE_NS;

/// The delay before the next try after `consecutive_failures` failures in a
/// row: 1 minute after the first, doubling with each further failure, at most
/// 24 hours. 0 when nothing has failed.
pub fn retry_delay_ns(consecutive_failures: u32) -> i64 {
    if consecutive_failures == 0 {
        return 0;
    }
    // 2^11 minutes exceed 24 hours, so a larger exponent changes nothing.
    let doublings = (consecutive_failures - 1).min(11);
    (MINUTE_NS << doublings).min(DAY_NS)
}

/// A failed attempt to read a disclosure in full or to download an
/// attachment, as the job that made it reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    /// A retry cannot change the result, such as a 404 from the exchange.
    pub permanent: bool,
    pub detail: String,
}

/// Whether a disclosure or an attachment is tried again after a recorded
/// failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// The next try is not before this instant, in nanoseconds since
    /// 1970-01-01 UTC.
    At { at_ns: i64 },
    /// It is not tried again: it is marked failed.
    Never,
}

impl Retry {
    fn after(next: &AfterFailure, already_failed: bool) -> Self {
        if next.exhausted || already_failed {
            Self::Never
        } else {
            Self::At {
                at_ns: next.retry_after_ns,
            }
        }
    }
}

/// The sizes of the queues of the jobs, and the counts of what failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::FromRow)]
pub struct QueueSizes {
    /// Disclosures to read in full, whatever their next try's time.
    pub unread_disclosures: i64,
    /// Attachments in state PENDING, whatever their next try's time.
    pub pending_attachments: i64,
    /// Disclosures marked failed, which are not read again.
    pub failed_disclosures: i64,
    /// Attachments in state FAILED.
    pub failed_attachments: i64,
}

/// How many disclosures and attachments the store holds, by state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::FromRow)]
pub struct Counts {
    /// Every stored disclosure, test disclosures included.
    pub disclosures: i64,
    /// Disclosures to read in full, whatever their next try's time.
    pub disclosures_unread: i64,
    /// Disclosures marked failed, which are not read again.
    pub disclosures_failed: i64,
    pub attachments_pending: i64,
    pub attachments_stored: i64,
    pub attachments_failed: i64,
}

/// What a failed attempt was for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    /// Listing one day.
    List,
    /// Reading one disclosure in full.
    Read,
    /// Downloading one attachment.
    Attachment,
}

impl FailureStage {
    /// The value of the `stage` column.
    fn code(self) -> i64 {
        match self {
            Self::List => 1,
            Self::Read => 2,
            Self::Attachment => 3,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::List),
            2 => Some(Self::Read),
            3 => Some(Self::Attachment),
            _ => None,
        }
    }
}

/// A row of the failure log.
#[derive(Debug, Clone, PartialEq)]
pub struct FailureRecord {
    pub id: i64,
    pub at_ns: i64,
    pub stage: FailureStage,
    pub permanent: bool,
    /// 0 for a failure to list a day.
    pub message_id: i64,
    /// 0 unless `stage` is `Attachment`.
    pub attachment_id: i64,
    /// The Oslo calendar day listed, when `stage` is `List`.
    pub day: Option<Date>,
    pub detail: String,
}

/// An attachment to download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentRef {
    pub message_id: i64,
    pub attachment_id: i64,
}

/// The result of listing one Oslo calendar day.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedDay {
    pub day: Date,
    pub listed_at_ns: i64,
    /// Rows on the list, `undecodable` included.
    pub rows: u32,
    /// The exchange truncated the list.
    pub overflow: bool,
    /// Rows on the list that did not decode.
    pub undecodable: u32,
    /// The day is known to be complete.
    pub sealed: bool,
}

/// Where a list of disclosures continues: after the disclosure with this
/// publication time and id, in the list's order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub published_at_ns: i64,
    pub message_id: i64,
}

impl Position {
    pub fn of(disclosure: &Disclosure) -> Self {
        Self {
            published_at_ns: disclosure.published_at_ns,
            message_id: disclosure.message_id,
        }
    }
}

/// What is known of an attachment's bytes once they are stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFile {
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}

pub struct Store {
    pool: SqlitePool,
    /// The `seq` of the newest committed event.
    latest_seq: watch::Sender<i64>,
}

impl Store {
    /// Opens the database file, creating it and its tables when missing.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(WRITE_LOCK_WAIT)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        Self::migrated(pool).await
    }

    /// A database that lives as long as the store. One connection, since
    /// every connection to `:memory:` opens a database of its own, and the
    /// pool never closes it for being idle or old, since closing it discards
    /// the database.
    ///
    /// There is no server to lose, so the pool does not test the connection
    /// before use, and a caller waits for it as long as the other callers
    /// hold it, up to `IN_MEMORY_ACQUIRE_TIMEOUT`. On tokio's paused clock,
    /// which advances while the runtime waits for SQLite's thread, the
    /// default of 30 seconds can pass during one wait.
    pub async fn open_in_memory() -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .test_before_acquire(false)
            .acquire_timeout(IN_MEMORY_ACQUIRE_TIMEOUT)
            .connect_with(options)
            .await?;
        Self::migrated(pool).await
    }

    /// Starts a transaction that writes. It takes SQLite's write lock at
    /// once, waiting its turn behind the other jobs. A transaction that
    /// started by reading and asked for the lock at its first write would
    /// fail with "database is locked" whenever another job had committed in
    /// between.
    async fn begin_write(&self) -> Result<Transaction<'static, Sqlite>, StoreError> {
        Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?)
    }

    async fn migrated(pool: SqlitePool) -> Result<Self, StoreError> {
        sqlx::migrate!().run(&pool).await?;
        let seq = select_latest_seq(&pool).await?;
        Ok(Self {
            pool,
            latest_seq: watch::Sender::new(seq),
        })
    }

    /// A receiver of the `seq` of the newest event. It changes after each
    /// transaction that wrote an event commits, never before.
    pub fn watch_latest_seq(&self) -> watch::Receiver<i64> {
        self.latest_seq.subscribe()
    }

    /// Announces the newest event a committed transaction wrote, if any.
    fn announce(&self, written: Option<i64>) {
        if let Some(seq) = written {
            self.latest_seq.send_if_modified(|latest| {
                let newer = seq > *latest;
                if newer {
                    *latest = seq;
                }
                newer
            });
        }
    }

    /// Stores a row of the exchange's list. For a disclosure already stored,
    /// the listed fields are compared with the stored document's: when they
    /// differ, a list row is replaced with the new one, and a disclosure read
    /// in full is marked to be read in full again. Neither writes an event for
    /// the disclosure; the full read writes `REVISED` when the document
    /// changed. A new row that corrects a disclosure read in full writes
    /// `CORRECTED` for that one. Returns
    /// whether the disclosure was new to the store.
    pub async fn store_list_entry(&self, entry: &Fetched, now_ns: i64) -> Result<bool, StoreError> {
        let mut tx = self.begin_write().await?;
        let inserted = insert_disclosure(&mut tx, entry, false, now_ns).await?;
        let mut written = None;
        if inserted {
            written =
                write_corrected_event_for_original(&mut tx, &entry.disclosure, now_ns).await?;
        } else {
            let stored: StoredDocument = sqlx::query_as(
                "SELECT read_in_full, document FROM disclosures WHERE message_id = ?",
            )
            .bind(entry.disclosure.message_id)
            .fetch_one(&mut *tx)
            .await?;
            match listed_change(&stored, &entry.disclosure) {
                ListedChange::Unchanged => {}
                ListedChange::NewListRow => replace_list_row(&mut tx, entry).await?,
                ListedChange::Reread => {
                    sqlx::query("UPDATE disclosures SET reread = 1 WHERE message_id = ?")
                        .bind(entry.disclosure.message_id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
        tx.commit().await?;
        self.announce(written);
        Ok(inserted)
    }

    /// Stores a disclosure read in full, and writes the event for what
    /// changed: `DISCOVERED` on the first full read, `REVISED` when the
    /// exchange's document differs from the stored one, nothing when it is
    /// the same. In every case the disclosure leaves the queue of
    /// `unread_disclosures`, and its earlier failures no longer count.
    pub async fn store_message(&self, message: &Fetched, now_ns: i64) -> Result<(), StoreError> {
        let message_id = message.disclosure.message_id;
        let mut tx = self.begin_write().await?;

        let stored: Option<StoredDocument> =
            sqlx::query_as("SELECT read_in_full, document FROM disclosures WHERE message_id = ?")
                .bind(message_id)
                .fetch_optional(&mut *tx)
                .await?;
        let is_new = stored.is_none();
        let change = full_read(stored.as_ref(), &message.document);

        if is_new {
            insert_disclosure(&mut tx, message, true, now_ns).await?;
        } else if change != FullRead::Unchanged {
            update_disclosure(&mut tx, message).await?;
        }
        sqlx::query(
            "UPDATE disclosures
                SET reread = 0, attempts = 0, transient_failures = 0, retry_after_ns = 0,
                    last_error = ''
              WHERE message_id = ?",
        )
        .bind(message_id)
        .execute(&mut *tx)
        .await?;

        let written = match change {
            FullRead::Unchanged => None,
            FullRead::Revised => {
                insert_attachments(&mut tx, &message.disclosure).await?;
                write_event(&mut tx, event::Kind::Revised, message_id, 0, now_ns).await?
            }
            FullRead::First => {
                insert_attachments(&mut tx, &message.disclosure).await?;
                let discovered =
                    write_event(&mut tx, event::Kind::Discovered, message_id, 0, now_ns).await?;
                let original = if is_new {
                    write_corrected_event_for_original(&mut tx, &message.disclosure, now_ns).await?
                } else {
                    None
                };
                let corrected =
                    write_corrected_event_if_corrected(&mut tx, message_id, now_ns).await?;
                discovered.max(original).max(corrected)
            }
        };

        tx.commit().await?;
        self.announce(written);
        Ok(())
    }

    /// Up to `limit` disclosures to read in full, newest first: those never
    /// read in full and those marked to be read again, without the failed
    /// ones and those whose next try is after `now_ns`.
    pub async fn unread_disclosures(
        &self,
        now_ns: i64,
        limit: u32,
    ) -> Result<Vec<i64>, StoreError> {
        // The first two conditions repeat the WHERE clause of the index
        // `disclosures_unread`.
        let ids = sqlx::query_scalar(
            "SELECT message_id FROM disclosures
              WHERE (read_in_full = 0 OR reread = 1) AND failed = 0
                AND retry_after_ns <= ?
              ORDER BY published_at_ns DESC, message_id DESC
              LIMIT ?",
        )
        .bind(now_ns)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }

    /// Up to `limit` attachments to download: pending ones whose next try is
    /// not after `now_ns`, newest disclosure first.
    pub async fn pending_attachments(
        &self,
        now_ns: i64,
        limit: u32,
    ) -> Result<Vec<AttachmentRef>, StoreError> {
        // `state = 1` is PENDING, written as a literal to match the WHERE
        // clause of the index `attachments_pending`.
        let rows: Vec<AttachmentRefRow> = sqlx::query_as(
            "SELECT a.message_id, a.attachment_id
               FROM attachments a JOIN disclosures d ON d.message_id = a.message_id
              WHERE a.state = 1 AND a.retry_after_ns <= ?
              ORDER BY d.published_at_ns DESC, d.message_id DESC, a.attachment_id
              LIMIT ?",
        )
        .bind(now_ns)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| AttachmentRef {
                message_id: row.message_id,
                attachment_id: row.attachment_id,
            })
            .collect())
    }

    /// Logs a failed full read and delays the next one. The
    /// `MAX_PERMANENT_FAILURES`th permanent failure marks the disclosure
    /// failed; transient failures never do. Returns when the disclosure is
    /// tried again, or `Retry::Never` once it is marked failed.
    pub async fn record_read_failure(
        &self,
        message_id: i64,
        failure: &Failure,
        now_ns: i64,
    ) -> Result<Retry, StoreError> {
        let mut tx = self.begin_write().await?;
        let counts: Option<DisclosureRetryCounts> = sqlx::query_as(
            "SELECT failed, attempts, transient_failures FROM disclosures WHERE message_id = ?",
        )
        .bind(message_id)
        .fetch_optional(&mut *tx)
        .await?;
        let counts = counts.ok_or(StoreError::NoDisclosure { message_id })?;
        let next = after_failure(counts.counts, failure.permanent, now_ns);
        sqlx::query(
            "UPDATE disclosures
                SET attempts = ?, transient_failures = ?, retry_after_ns = ?,
                    failed = failed OR ?, last_error = ?, last_attempt_at_ns = ?
              WHERE message_id = ?",
        )
        .bind(next.counts.attempts)
        .bind(next.counts.transient_failures)
        .bind(next.retry_after_ns)
        .bind(next.exhausted)
        .bind(&failure.detail)
        .bind(now_ns)
        .bind(message_id)
        .execute(&mut *tx)
        .await?;
        let entry = FailureEntry {
            stage: FailureStage::Read,
            permanent: failure.permanent,
            message_id,
            attachment_id: 0,
            day: String::new(),
            detail: &failure.detail,
        };
        insert_failure(&mut tx, &entry, now_ns).await?;
        tx.commit().await?;
        Ok(Retry::after(&next, counts.failed))
    }

    /// Logs a failed download and delays the next one. The
    /// `MAX_PERMANENT_FAILURES`th permanent failure sets the attachment's
    /// state to FAILED and writes `ATTACHMENT_FAILED`, as
    /// `mark_attachment_failed` does; transient failures never do. Returns
    /// when the attachment is tried again, or `Retry::Never` once its state
    /// is FAILED.
    pub async fn record_attachment_failure(
        &self,
        message_id: i64,
        attachment_id: i64,
        failure: &Failure,
        now_ns: i64,
    ) -> Result<Retry, StoreError> {
        let mut tx = self.begin_write().await?;
        let counts: Option<AttachmentRetryCounts> = sqlx::query_as(
            "SELECT state, attempts, transient_failures FROM attachments
              WHERE message_id = ? AND attachment_id = ?",
        )
        .bind(message_id)
        .bind(attachment_id)
        .fetch_optional(&mut *tx)
        .await?;
        let counts = counts.ok_or(StoreError::NoAttachment {
            message_id,
            attachment_id,
        })?;
        let already_failed = counts.state == attachment::State::Failed as i32;
        let next = after_failure(counts.counts, failure.permanent, now_ns);
        sqlx::query(
            "UPDATE attachments
                SET attempts = ?, transient_failures = ?, retry_after_ns = ?,
                    last_error = ?, last_attempt_at_ns = ?
              WHERE message_id = ? AND attachment_id = ?",
        )
        .bind(next.counts.attempts)
        .bind(next.counts.transient_failures)
        .bind(next.retry_after_ns)
        .bind(&failure.detail)
        .bind(now_ns)
        .bind(message_id)
        .bind(attachment_id)
        .execute(&mut *tx)
        .await?;
        let written = if next.exhausted && !already_failed {
            set_attachment_failed(&mut tx, message_id, attachment_id, now_ns).await?
        } else {
            None
        };
        let entry = FailureEntry {
            stage: FailureStage::Attachment,
            permanent: failure.permanent,
            message_id,
            attachment_id,
            day: String::new(),
            detail: &failure.detail,
        };
        insert_failure(&mut tx, &entry, now_ns).await?;
        tx.commit().await?;
        self.announce(written);
        Ok(Retry::after(&next, already_failed))
    }

    /// The sizes of the queues of `reader` and `downloader`, and the counts
    /// of disclosures and attachments marked failed.
    pub async fn queue_sizes(&self) -> Result<QueueSizes, StoreError> {
        let counts = self.counts().await?;
        Ok(QueueSizes {
            unread_disclosures: counts.disclosures_unread,
            pending_attachments: counts.attachments_pending,
            failed_disclosures: counts.disclosures_failed,
            failed_attachments: counts.attachments_failed,
        })
    }

    /// How many disclosures and attachments the store holds, by state.
    pub async fn counts(&self) -> Result<Counts, StoreError> {
        // The unread and pending conditions repeat the WHERE clauses of the
        // indexes `disclosures_unread` and `attachments_pending`; 1 is
        // PENDING, 2 is STORED and 3 is FAILED.
        let counts = sqlx::query_as(
            "SELECT
               (SELECT count(*) FROM disclosures) AS disclosures,
               (SELECT count(*) FROM disclosures
                 WHERE (read_in_full = 0 OR reread = 1) AND failed = 0) AS disclosures_unread,
               (SELECT count(*) FROM disclosures WHERE failed = 1) AS disclosures_failed,
               (SELECT count(*) FROM attachments WHERE state = 1) AS attachments_pending,
               (SELECT count(*) FROM attachments WHERE state = 2) AS attachments_stored,
               (SELECT count(*) FROM attachments WHERE state = 3) AS attachments_failed",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(counts)
    }

    /// Logs a failure to list one Oslo calendar day.
    pub async fn record_list_failure(
        &self,
        day: Date,
        detail: &str,
        now_ns: i64,
    ) -> Result<(), StoreError> {
        let entry = FailureEntry {
            stage: FailureStage::List,
            permanent: false,
            message_id: 0,
            attachment_id: 0,
            day: day.to_string(),
            detail,
        };
        let mut connection = self.pool.acquire().await?;
        insert_failure(&mut connection, &entry, now_ns).await
    }

    /// Up to `limit` rows of the failure log, newest first.
    pub async fn failures(&self, limit: u32) -> Result<Vec<FailureRecord>, StoreError> {
        self.failures_after(None, None, limit).await
    }

    /// Up to `limit` rows of the failure log, newest first: with
    /// `message_id`, only the failures of that disclosure and its
    /// attachments; with `after_id`, only those older than that row, to
    /// continue where the previous call ended.
    pub async fn failures_after(
        &self,
        message_id: Option<i64>,
        after_id: Option<i64>,
        limit: u32,
    ) -> Result<Vec<FailureRecord>, StoreError> {
        let rows: Vec<FailureRow> = sqlx::query_as(
            "SELECT id, at_ns, stage, permanent, message_id, attachment_id, day, detail
               FROM failures
              WHERE (?1 IS NULL OR message_id = ?1) AND id < ?2
              ORDER BY id DESC LIMIT ?3",
        )
        .bind(message_id)
        .bind(after_id.unwrap_or(i64::MAX))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(FailureRecord::try_from).collect()
    }

    /// Records the result of listing a day, replacing an earlier one.
    pub async fn record_listed_day(&self, listed: &ListedDay) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT OR REPLACE INTO listed_days
                    (day, listed_at_ns, rows, overflow, undecodable, sealed)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(listed.day.to_string())
        .bind(listed.listed_at_ns)
        .bind(listed.rows)
        .bind(listed.overflow)
        .bind(listed.undecodable)
        .bind(listed.sealed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The listed days from `from` to `to`, both included, in ascending order.
    pub async fn listed_days(&self, from: Date, to: Date) -> Result<Vec<ListedDay>, StoreError> {
        // `YYYY-MM-DD` text orders as the dates do.
        let rows: Vec<ListedDayRow> = sqlx::query_as(
            "SELECT day, listed_at_ns, rows, overflow, undecodable, sealed
               FROM listed_days WHERE day BETWEEN ? AND ? ORDER BY day",
        )
        .bind(from.to_string())
        .bind(to.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(ListedDay::try_from).collect()
    }

    /// Records that an attachment's bytes are stored, and writes
    /// `ATTACHMENT_STORED`.
    pub async fn mark_attachment_stored(
        &self,
        message_id: i64,
        attachment_id: i64,
        file: &StoredFile,
        now_ns: i64,
    ) -> Result<(), StoreError> {
        let mut tx = self.begin_write().await?;
        let updated = sqlx::query(
            "UPDATE attachments SET state = ?, content_type = ?, size_bytes = ?, sha256 = ?
             WHERE message_id = ? AND attachment_id = ?",
        )
        .bind(attachment::State::Stored as i32)
        .bind(&file.content_type)
        .bind(file.size_bytes)
        .bind(&file.sha256)
        .bind(message_id)
        .bind(attachment_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NoAttachment {
                message_id,
                attachment_id,
            });
        }
        let kind = event::Kind::AttachmentStored;
        let written = write_event(&mut tx, kind, message_id, attachment_id, now_ns).await?;
        tx.commit().await?;
        self.announce(written);
        Ok(())
    }

    /// Records that an attachment will not be requested again, and writes
    /// `ATTACHMENT_FAILED`.
    pub async fn mark_attachment_failed(
        &self,
        message_id: i64,
        attachment_id: i64,
        now_ns: i64,
    ) -> Result<(), StoreError> {
        let mut tx = self.begin_write().await?;
        let written = set_attachment_failed(&mut tx, message_id, attachment_id, now_ns).await?;
        tx.commit().await?;
        self.announce(written);
        Ok(())
    }

    /// One disclosure, test disclosures included.
    pub async fn disclosure(&self, message_id: i64) -> Result<Option<Disclosure>, StoreError> {
        let mut connection = self.pool.acquire().await?;
        load_disclosure(&mut connection, message_id).await
    }

    /// Up to `limit` disclosures the filter matches, newest first: by
    /// publication time, then by message id, both descending. Test
    /// disclosures are left out. `after` continues a list where the previous
    /// call ended. A disclosure whose document does not decode is left out
    /// and logged.
    pub async fn disclosures(
        &self,
        filter: &DisclosureFilter,
        after: Option<Position>,
        limit: u32,
    ) -> Result<Vec<Disclosure>, StoreError> {
        let signs = serde_json::to_string(&filter.issuer_signs).unwrap_or_default();
        let after = after.unwrap_or(Position {
            published_at_ns: i64::MAX,
            message_id: i64::MAX,
        });
        let mut connection = self.pool.acquire().await?;
        let rows: Vec<DisclosureRow> = sqlx::query_as(
            "SELECT message_id, document, read_in_full, received_at_ns, failed,
                    (SELECT max(c.message_id) FROM disclosures c
                      WHERE c.correction_for_message_id = d.message_id)
                        AS corrected_by_message_id
               FROM disclosures d
              WHERE is_test = 0
                AND (?1 = 0 OR published_at_ns >= ?1)
                AND (?2 = 0 OR published_at_ns < ?2)
                AND (?3 = 0 OR issuer_sign IN (SELECT value FROM json_each(?4)))
                AND (published_at_ns, message_id) < (?5, ?6)
              ORDER BY published_at_ns DESC, message_id DESC
              LIMIT ?7",
        )
        .bind(filter.published_from_ns)
        .bind(filter.published_before_ns)
        .bind(filter.issuer_signs.len() as i64)
        .bind(signs)
        .bind(after.published_at_ns)
        .bind(after.message_id)
        .bind(limit)
        .fetch_all(&mut *connection)
        .await?;

        let mut disclosures = Vec::with_capacity(rows.len());
        for row in rows {
            let message_id = row.message_id;
            let attachments = attachment_states(&mut connection, message_id).await?;
            match decode_row(row, attachments) {
                Ok(disclosure) => disclosures.push(disclosure),
                Err(error) => tracing::warn!(message_id, %error, "left out of a list"),
            }
        }
        Ok(disclosures)
    }

    /// Up to `limit` events after `seq`, in order.
    pub async fn events_after(&self, seq: i64, limit: u32) -> Result<Vec<Event>, StoreError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT seq, kind, at_ns, attachment_id, disclosure FROM events
              WHERE seq > ? ORDER BY seq LIMIT ?",
        )
        .bind(seq)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let seq = row.seq;
                let disclosure = Disclosure::decode(row.disclosure.as_slice())
                    .map_err(|source| StoreError::EventDisclosure { seq, source })?;
                Ok(Event {
                    seq,
                    kind: row.kind,
                    at_ns: row.at_ns,
                    disclosure: Some(disclosure),
                    attachment_id: row.attachment_id,
                })
            })
            .collect()
    }

    /// The `seq` of the newest event; 0 when the log is empty.
    pub async fn latest_seq(&self) -> Result<i64, StoreError> {
        select_latest_seq(&self.pool).await
    }

    /// The `seq` of the last event delivered to the consumer `name`; `None`
    /// when no position is stored for it.
    pub async fn delivery_position(&self, name: &str) -> Result<Option<i64>, StoreError> {
        let seq = sqlx::query_scalar("SELECT seq FROM delivery_positions WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(seq)
    }

    /// Stores `seq` as the last event delivered to the consumer `name`,
    /// replacing its earlier position.
    pub async fn set_delivery_position(&self, name: &str, seq: i64) -> Result<(), StoreError> {
        sqlx::query("INSERT OR REPLACE INTO delivery_positions (name, seq) VALUES (?, ?)")
            .bind(name)
            .bind(seq)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn select_latest_seq(pool: &SqlitePool) -> Result<i64, StoreError> {
    let seq = sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM events")
        .fetch_one(pool)
        .await?;
    Ok(seq)
}

/// What storing a disclosure read in full changes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FullRead {
    /// The store held no disclosure with the id, or only its list row.
    First,
    /// The exchange's document differs from the stored one.
    Revised,
    Unchanged,
}

fn full_read(stored: Option<&StoredDocument>, document: &str) -> FullRead {
    match stored {
        Some(stored) if !stored.read_in_full => FullRead::First,
        Some(stored) if stored.document != document => FullRead::Revised,
        Some(_) => FullRead::Unchanged,
        None => FullRead::First,
    }
}

/// What storing a row of the list changes for a disclosure already stored.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ListedChange {
    /// The listed fields are those of the stored document.
    Unchanged,
    /// The stored list row is replaced with the new one.
    NewListRow,
    /// The disclosure is read in full and is marked to be read again.
    Reread,
}

/// Compares the listed fields of a row of the list with those of the stored
/// document, which is read as a list row whether or not it is the full
/// message. A stored document that does not decode counts as changed.
fn listed_change(stored: &StoredDocument, listed: &Disclosure) -> ListedChange {
    let unchanged = exchange::decode_document(&stored.document, false)
        .is_ok_and(|stored_row| stored_row == *listed);
    match (unchanged, stored.read_in_full) {
        (true, _) => ListedChange::Unchanged,
        (false, false) => ListedChange::NewListRow,
        (false, true) => ListedChange::Reread,
    }
}

/// The failure counters of a disclosure or an attachment.
#[derive(Debug, Clone, Copy, PartialEq, sqlx::FromRow)]
struct RetryCounts {
    attempts: u32,
    transient_failures: u32,
}

/// The failure counters after one more failure, and when to try again.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AfterFailure {
    counts: RetryCounts,
    retry_after_ns: i64,
    /// The failure was permanent, and the permanent failures reached
    /// `MAX_PERMANENT_FAILURES`.
    exhausted: bool,
}

/// A transient failure adds to the run of transient failures and leaves the
/// permanent ones as they are. A permanent failure adds to the permanent
/// ones and ends the run of transient ones. Either delays the next try by
/// `retry_delay_ns` of the count it raised.
fn after_failure(current: RetryCounts, permanent: bool, now_ns: i64) -> AfterFailure {
    let counts = if permanent {
        RetryCounts {
            attempts: current.attempts.saturating_add(1),
            transient_failures: 0,
        }
    } else {
        RetryCounts {
            attempts: current.attempts,
            transient_failures: current.transient_failures.saturating_add(1),
        }
    };
    let raised = if permanent {
        counts.attempts
    } else {
        counts.transient_failures
    };
    AfterFailure {
        counts,
        retry_after_ns: now_ns.saturating_add(retry_delay_ns(raised)),
        exhausted: permanent && counts.attempts >= MAX_PERMANENT_FAILURES,
    }
}

/// What `store_message` and `store_list_entry` compare an incoming
/// disclosure with.
#[derive(Debug, sqlx::FromRow)]
struct StoredDocument {
    read_in_full: bool,
    document: String,
}

/// The failure counters of a disclosure, and whether it is marked failed.
#[derive(Debug, sqlx::FromRow)]
struct DisclosureRetryCounts {
    failed: bool,
    #[sqlx(flatten)]
    counts: RetryCounts,
}

/// The failure counters of an attachment, and its state.
#[derive(Debug, sqlx::FromRow)]
struct AttachmentRetryCounts {
    state: i32,
    #[sqlx(flatten)]
    counts: RetryCounts,
}

/// A row of the queue of `pending_attachments`.
#[derive(Debug, sqlx::FromRow)]
struct AttachmentRefRow {
    message_id: i64,
    attachment_id: i64,
}

/// A row of `failures` as `failures` selects it.
#[derive(Debug, sqlx::FromRow)]
struct FailureRow {
    id: i64,
    at_ns: i64,
    stage: i64,
    permanent: bool,
    message_id: i64,
    attachment_id: i64,
    day: String,
    detail: String,
}

impl TryFrom<FailureRow> for FailureRecord {
    type Error = StoreError;

    fn try_from(row: FailureRow) -> Result<Self, StoreError> {
        let stage = FailureStage::from_code(row.stage).ok_or(StoreError::FailureStage {
            id: row.id,
            stage: row.stage,
        })?;
        let day = match row.day.as_str() {
            "" => None,
            _ => Some(parse_day(row.day)?),
        };
        Ok(Self {
            id: row.id,
            at_ns: row.at_ns,
            stage,
            permanent: row.permanent,
            message_id: row.message_id,
            attachment_id: row.attachment_id,
            day,
            detail: row.detail,
        })
    }
}

/// A row of `failures` to insert.
struct FailureEntry<'a> {
    stage: FailureStage,
    permanent: bool,
    message_id: i64,
    attachment_id: i64,
    /// `YYYY-MM-DD`, or '' when the stage is not `List`.
    day: String,
    detail: &'a str,
}

/// A row of `listed_days`.
#[derive(Debug, sqlx::FromRow)]
struct ListedDayRow {
    day: String,
    listed_at_ns: i64,
    rows: u32,
    overflow: bool,
    undecodable: u32,
    sealed: bool,
}

impl TryFrom<ListedDayRow> for ListedDay {
    type Error = StoreError;

    fn try_from(row: ListedDayRow) -> Result<Self, StoreError> {
        Ok(Self {
            day: parse_day(row.day)?,
            listed_at_ns: row.listed_at_ns,
            rows: row.rows,
            overflow: row.overflow,
            undecodable: row.undecodable,
            sealed: row.sealed,
        })
    }
}

fn parse_day(value: String) -> Result<Date, StoreError> {
    value
        .parse()
        .map_err(|source| StoreError::Day { value, source })
}

/// A row of `disclosures` as the reads select it.
#[derive(Debug, sqlx::FromRow)]
struct DisclosureRow {
    message_id: i64,
    document: String,
    read_in_full: bool,
    received_at_ns: i64,
    /// The full read failed permanently and is not tried again.
    failed: bool,
    /// The newest stored disclosure that corrects this one.
    corrected_by_message_id: Option<i64>,
}

/// A row of `attachments`.
#[derive(Debug, sqlx::FromRow)]
struct AttachmentRow {
    attachment_id: i64,
    state: i32,
    content_type: String,
    size_bytes: i64,
    sha256: String,
}

/// A row of `events`. `disclosure` is the encoded `Disclosure`.
#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    seq: i64,
    kind: i32,
    at_ns: i64,
    attachment_id: i64,
    disclosure: Vec<u8>,
}

/// Inserts the disclosure unless one with its id is stored. Returns whether
/// it inserted.
async fn insert_disclosure(
    connection: &mut SqliteConnection,
    fetched: &Fetched,
    read_in_full: bool,
    now_ns: i64,
) -> Result<bool, StoreError> {
    let disclosure = &fetched.disclosure;
    let inserted = sqlx::query(
        "INSERT INTO disclosures (message_id, published_at_ns, received_at_ns, issuer_sign,
                                  is_test, correction_for_message_id, read_in_full, document)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (message_id) DO NOTHING",
    )
    .bind(disclosure.message_id)
    .bind(disclosure.published_at_ns)
    .bind(now_ns)
    .bind(issuer_sign(disclosure))
    .bind(disclosure.test)
    .bind(disclosure.correction_for_message_id)
    .bind(read_in_full)
    .bind(&fetched.document)
    .execute(connection)
    .await?;
    Ok(inserted.rows_affected() == 1)
}

/// Replaces a stored disclosure with the full message. `received_at_ns` stays.
async fn update_disclosure(
    connection: &mut SqliteConnection,
    message: &Fetched,
) -> Result<(), StoreError> {
    let disclosure = &message.disclosure;
    sqlx::query(
        "UPDATE disclosures
            SET published_at_ns = ?, issuer_sign = ?, is_test = ?,
                correction_for_message_id = ?, read_in_full = 1, document = ?
          WHERE message_id = ?",
    )
    .bind(disclosure.published_at_ns)
    .bind(issuer_sign(disclosure))
    .bind(disclosure.test)
    .bind(disclosure.correction_for_message_id)
    .bind(&message.document)
    .bind(disclosure.message_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Replaces a stored list row with a newer one. `received_at_ns` stays.
async fn replace_list_row(
    connection: &mut SqliteConnection,
    entry: &Fetched,
) -> Result<(), StoreError> {
    let disclosure = &entry.disclosure;
    sqlx::query(
        "UPDATE disclosures
            SET published_at_ns = ?, issuer_sign = ?, is_test = ?,
                correction_for_message_id = ?, document = ?
          WHERE message_id = ? AND read_in_full = 0",
    )
    .bind(disclosure.published_at_ns)
    .bind(issuer_sign(disclosure))
    .bind(disclosure.test)
    .bind(disclosure.correction_for_message_id)
    .bind(&entry.document)
    .bind(disclosure.message_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Sets an attachment's state to FAILED and writes `ATTACHMENT_FAILED`.
/// Returns the event's `seq`.
async fn set_attachment_failed(
    connection: &mut SqliteConnection,
    message_id: i64,
    attachment_id: i64,
    now_ns: i64,
) -> Result<Option<i64>, StoreError> {
    let updated =
        sqlx::query("UPDATE attachments SET state = ? WHERE message_id = ? AND attachment_id = ?")
            .bind(attachment::State::Failed as i32)
            .bind(message_id)
            .bind(attachment_id)
            .execute(&mut *connection)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(StoreError::NoAttachment {
            message_id,
            attachment_id,
        });
    }
    let kind = event::Kind::AttachmentFailed;
    write_event(connection, kind, message_id, attachment_id, now_ns).await
}

async fn insert_failure(
    connection: &mut SqliteConnection,
    entry: &FailureEntry<'_>,
    now_ns: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO failures (at_ns, stage, permanent, message_id, attachment_id, day, detail)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(now_ns)
    .bind(entry.stage.code())
    .bind(entry.permanent)
    .bind(entry.message_id)
    .bind(entry.attachment_id)
    .bind(&entry.day)
    .bind(entry.detail)
    .execute(connection)
    .await?;
    Ok(())
}

fn issuer_sign(disclosure: &Disclosure) -> &str {
    disclosure
        .issuer
        .as_ref()
        .map_or("", |issuer| issuer.sign.as_str())
}

/// Adds a pending row for each attachment the store does not have. The state
/// of one already stored is kept.
async fn insert_attachments(
    connection: &mut SqliteConnection,
    disclosure: &Disclosure,
) -> Result<(), StoreError> {
    for attachment in &disclosure.attachments {
        sqlx::query(
            "INSERT INTO attachments (message_id, attachment_id, state) VALUES (?, ?, ?)
             ON CONFLICT (message_id, attachment_id) DO NOTHING",
        )
        .bind(disclosure.message_id)
        .bind(attachment.id)
        .bind(attachment::State::Pending as i32)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Writes an event that carries the disclosure as it now stands. Returns the
/// event's `seq`; `None` when no disclosure has the id and nothing is written.
async fn write_event(
    connection: &mut SqliteConnection,
    kind: event::Kind,
    message_id: i64,
    attachment_id: i64,
    now_ns: i64,
) -> Result<Option<i64>, StoreError> {
    let Some(disclosure) = load_disclosure(connection, message_id).await? else {
        return Ok(None);
    };
    let inserted = sqlx::query(
        "INSERT INTO events (kind, at_ns, message_id, attachment_id, disclosure)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(kind as i32)
    .bind(now_ns)
    .bind(message_id)
    .bind(attachment_id)
    .bind(disclosure.encode_to_vec())
    .execute(connection)
    .await?;
    Ok(Some(inserted.last_insert_rowid()))
}

/// A correction was stored for the first time: writes `CORRECTED` for the
/// disclosure it corrects, when that one has been read in full. When it has
/// not, `write_corrected_event_if_corrected` writes the event at its full
/// read, so each pair gets one event whichever of the two arrives first.
/// Returns the event's `seq`, when one is written.
async fn write_corrected_event_for_original(
    connection: &mut SqliteConnection,
    correction: &Disclosure,
    now_ns: i64,
) -> Result<Option<i64>, StoreError> {
    let original = correction.correction_for_message_id;
    if original == 0 {
        return Ok(None);
    }
    let read_in_full: Option<bool> =
        sqlx::query_scalar("SELECT read_in_full FROM disclosures WHERE message_id = ?")
            .bind(original)
            .fetch_optional(&mut *connection)
            .await?;
    if read_in_full == Some(true) {
        return write_event(connection, event::Kind::Corrected, original, 0, now_ns).await;
    }
    Ok(None)
}

/// A disclosure was read in full for the first time: writes `CORRECTED` for
/// it when a stored disclosure corrects it. Returns the event's `seq`, when
/// one is written.
async fn write_corrected_event_if_corrected(
    connection: &mut SqliteConnection,
    message_id: i64,
    now_ns: i64,
) -> Result<Option<i64>, StoreError> {
    let corrected: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM disclosures WHERE correction_for_message_id = ?)",
    )
    .bind(message_id)
    .fetch_one(&mut *connection)
    .await?;
    if corrected {
        return write_event(connection, event::Kind::Corrected, message_id, 0, now_ns).await;
    }
    Ok(None)
}

async fn load_disclosure(
    connection: &mut SqliteConnection,
    message_id: i64,
) -> Result<Option<Disclosure>, StoreError> {
    let row: Option<DisclosureRow> = sqlx::query_as(
        "SELECT message_id, document, read_in_full, received_at_ns, failed,
                (SELECT max(c.message_id) FROM disclosures c
                  WHERE c.correction_for_message_id = d.message_id)
                    AS corrected_by_message_id
           FROM disclosures d WHERE message_id = ?",
    )
    .bind(message_id)
    .fetch_optional(&mut *connection)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let attachments = attachment_states(connection, message_id).await?;
    decode_row(row, attachments).map(Some)
}

/// The stored state of each attachment of a disclosure, by attachment id.
async fn attachment_states(
    connection: &mut SqliteConnection,
    message_id: i64,
) -> Result<HashMap<i64, Attachment>, StoreError> {
    let rows: Vec<AttachmentRow> = sqlx::query_as(
        "SELECT attachment_id, state, content_type, size_bytes, sha256
           FROM attachments WHERE message_id = ?",
    )
    .bind(message_id)
    .fetch_all(connection)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let stored = Attachment {
                id: row.attachment_id,
                state: row.state,
                content_type: row.content_type,
                size_bytes: row.size_bytes,
                sha256: row.sha256,
                ..Default::default()
            };
            (row.attachment_id, stored)
        })
        .collect())
}

/// The disclosure of a row: its document decoded, then what only the store
/// holds added to it.
fn decode_row(
    row: DisclosureRow,
    mut stored_attachments: HashMap<i64, Attachment>,
) -> Result<Disclosure, StoreError> {
    let mut disclosure =
        exchange::decode_document(&row.document, row.read_in_full).map_err(|source| {
            StoreError::Document {
                message_id: row.message_id,
                source,
            }
        })?;
    disclosure.received_at_ns = row.received_at_ns;
    disclosure.corrected_by_message_id = row.corrected_by_message_id.unwrap_or_default();
    disclosure.read_failed = row.failed;
    for attachment in &mut disclosure.attachments {
        if let Some(stored) = stored_attachments.remove(&attachment.id) {
            *attachment = Attachment {
                name: std::mem::take(&mut attachment.name),
                ..stored
            };
        }
    }
    Ok(disclosure)
}

// The unit tests of the store's pure functions. Everything that runs SQL is
// tested in tests/store.rs.
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"{
        "messageId": 7, "publishedTime": "2026-09-22T12:00:00Z", "title": "Results",
        "attachments": [{"id": 9, "name": "q2.pdf"}]
    }"#;
    const RECEIVED_AT_NS: i64 = 1_790_078_460_000_000_000;

    fn row(document: &str, corrected_by_message_id: Option<i64>) -> DisclosureRow {
        DisclosureRow {
            message_id: 7,
            document: document.to_owned(),
            read_in_full: true,
            received_at_ns: RECEIVED_AT_NS,
            failed: false,
            corrected_by_message_id,
        }
    }

    fn stored(read_in_full: bool, document: &str) -> StoredDocument {
        StoredDocument {
            read_in_full,
            document: document.to_owned(),
        }
    }

    #[test]
    fn full_read__stored_states__names_the_change() {
        let cases = [
            (None, FullRead::First),
            (Some(stored(false, "list row")), FullRead::First),
            (Some(stored(true, "old document")), FullRead::Revised),
            (Some(stored(true, "new document")), FullRead::Unchanged),
        ];
        for (stored, expected) in cases {
            assert_eq!(
                full_read(stored.as_ref(), "new document"),
                expected,
                "stored: {stored:?}"
            );
        }
    }

    #[test]
    fn retry_delay_ns__consecutive_failures__doubles_from_one_minute_up_to_a_day() {
        const MINUTE: i64 = 60_000_000_000;
        const DAY: i64 = 1_440 * MINUTE;
        let cases = [
            (0, 0),
            (1, MINUTE),
            (2, 2 * MINUTE),
            (3, 4 * MINUTE),
            (11, 1_024 * MINUTE),
            (12, DAY),
            (64, DAY),
            (u32::MAX, DAY),
        ];
        for (failures, expected) in cases {
            assert_eq!(retry_delay_ns(failures), expected, "failures: {failures}");
        }
    }

    const LIST_ROW: &str =
        r#"{"messageId": 7, "publishedTime": "2026-09-22T12:00:00Z", "title": "Results"}"#;
    const RETITLED_LIST_ROW: &str = r#"{"messageId": 7, "publishedTime": "2026-09-22T12:00:00Z", "title": "Results (updated)"}"#;

    fn listed(document: &str) -> Disclosure {
        exchange::decode_document(document, false).unwrap()
    }

    #[test]
    fn listed_change__stored_document_and_new_row__names_the_change() {
        let cases = [
            (stored(false, LIST_ROW), LIST_ROW, ListedChange::Unchanged),
            (
                stored(false, LIST_ROW),
                RETITLED_LIST_ROW,
                ListedChange::NewListRow,
            ),
            (
                stored(true, DOCUMENT),
                RETITLED_LIST_ROW,
                ListedChange::Reread,
            ),
            (
                stored(false, "not json"),
                LIST_ROW,
                ListedChange::NewListRow,
            ),
            (stored(true, "not json"), LIST_ROW, ListedChange::Reread),
        ];
        for (stored, row, expected) in cases {
            assert_eq!(
                listed_change(&stored, &listed(row)),
                expected,
                "stored: {stored:?}, row: {row}"
            );
        }
    }

    #[test]
    fn listed_change__full_message_with_body_and_attachments__compares_only_listed_fields() {
        let stored = stored(true, DOCUMENT);

        let change = listed_change(&stored, &listed(LIST_ROW));

        assert_eq!(change, ListedChange::Unchanged);
    }

    const NOW_NS: i64 = 1_790_078_460_000_000_000;
    const MINUTE_LATER_NS: i64 = NOW_NS + 60_000_000_000;

    fn counts(attempts: u32, transient_failures: u32) -> RetryCounts {
        RetryCounts {
            attempts,
            transient_failures,
        }
    }

    #[test]
    fn after_failure__transient__adds_to_transient_failures_and_keeps_attempts() {
        let next = after_failure(counts(2, 5), false, NOW_NS);

        assert_eq!(next.counts, counts(2, 6));
    }

    #[test]
    fn after_failure__permanent__adds_to_attempts_and_ends_the_transient_run() {
        let next = after_failure(counts(1, 5), true, NOW_NS);

        assert_eq!(next.counts, counts(2, 0));
    }

    #[test]
    fn after_failure__first_transient__retries_a_minute_later() {
        let next = after_failure(counts(0, 0), false, NOW_NS);

        assert_eq!(next.retry_after_ns, MINUTE_LATER_NS);
    }

    #[test]
    fn after_failure__first_permanent__retries_a_minute_later() {
        let next = after_failure(counts(0, 7), true, NOW_NS);

        assert_eq!(next.retry_after_ns, MINUTE_LATER_NS);
    }

    #[test]
    fn after_failure__counts_and_kind__exhausted_only_at_the_maximum_permanent_failure() {
        let cases = [
            (counts(0, 0), true, false),
            (counts(1, 0), true, false),
            (counts(2, 0), true, true),
            (counts(3, 0), true, true),
            (counts(2, 0), false, false),
            (counts(0, u32::MAX), false, false),
            (counts(3, 0), false, false),
        ];
        for (current, permanent, expected) in cases {
            assert_eq!(
                after_failure(current, permanent, NOW_NS).exhausted,
                expected,
                "counts: {current:?}, permanent: {permanent}"
            );
        }
    }

    #[test]
    fn after_failure__retry_time_beyond_i64__saturates() {
        let next = after_failure(counts(0, 0), false, i64::MAX);

        assert_eq!(next.retry_after_ns, i64::MAX);
    }

    fn failure_row(stage: i64, day: &str) -> FailureRow {
        FailureRow {
            id: 4,
            at_ns: NOW_NS,
            stage,
            permanent: false,
            message_id: 0,
            attachment_id: 0,
            day: day.to_owned(),
            detail: "HTTP 503".to_owned(),
        }
    }

    #[test]
    fn failure_record_try_from__stage_codes__returns_the_stage() {
        let cases = [
            (1, FailureStage::List),
            (2, FailureStage::Read),
            (3, FailureStage::Attachment),
        ];
        for (code, expected) in cases {
            assert_eq!(
                FailureRecord::try_from(failure_row(code, ""))
                    .unwrap()
                    .stage,
                expected,
                "code: {code}"
            );
        }
    }

    #[test]
    fn failure_stage_code__every_stage__reads_back_as_itself() {
        let stages = [
            FailureStage::List,
            FailureStage::Read,
            FailureStage::Attachment,
        ];
        for stage in stages {
            assert_eq!(FailureStage::from_code(stage.code()), Some(stage));
        }
    }

    #[test]
    fn failure_record_try_from__unknown_stage__returns_error_naming_row_and_stage() {
        let error = FailureRecord::try_from(failure_row(9, "")).unwrap_err();

        assert_eq!(error.to_string(), "failure 4: stage 9 is not a stage");
    }

    #[test]
    fn failure_record_try_from__day_set__returns_the_date() {
        let record = FailureRecord::try_from(failure_row(1, "2026-09-22")).unwrap();

        assert_eq!(record.day, Some(Date::constant(2026, 9, 22)));
    }

    #[test]
    fn failure_record_try_from__day_empty__returns_no_date() {
        let record = FailureRecord::try_from(failure_row(2, "")).unwrap();

        assert_eq!(record.day, None);
    }

    fn listed_day_row(day: &str) -> ListedDayRow {
        ListedDayRow {
            day: day.to_owned(),
            listed_at_ns: NOW_NS,
            rows: 214,
            overflow: false,
            undecodable: 1,
            sealed: true,
        }
    }

    #[test]
    fn listed_day_try_from__row__returns_the_day_and_its_counts() {
        let listed = ListedDay::try_from(listed_day_row("2026-09-22")).unwrap();

        let expected = ListedDay {
            day: Date::constant(2026, 9, 22),
            listed_at_ns: NOW_NS,
            rows: 214,
            overflow: false,
            undecodable: 1,
            sealed: true,
        };
        assert_eq!(listed, expected);
    }

    #[test]
    fn listed_day_try_from__day_not_a_date__returns_error_naming_the_value() {
        let error = ListedDay::try_from(listed_day_row("22.09.2026")).unwrap_err();

        assert_eq!(
            error.to_string(),
            r#"the stored day "22.09.2026" is not a date"#
        );
    }

    #[test]
    fn decode_row__row_of_the_store__adds_received_time_and_correction() {
        let disclosure = decode_row(row(DOCUMENT, Some(8)), HashMap::new()).unwrap();

        assert_eq!(disclosure.received_at_ns, RECEIVED_AT_NS);
        assert_eq!(disclosure.corrected_by_message_id, 8);
    }

    #[test]
    fn decode_row__no_correction_stored__leaves_corrected_by_unset() {
        let disclosure = decode_row(row(DOCUMENT, None), HashMap::new()).unwrap();

        assert_eq!(disclosure.corrected_by_message_id, 0);
    }

    #[test]
    fn decode_row__attachment_stored__takes_its_state_and_keeps_its_name() {
        let stored = Attachment {
            id: 9,
            state: attachment::State::Stored as i32,
            content_type: "application/pdf".into(),
            size_bytes: 84_743,
            ..Default::default()
        };

        let disclosure = decode_row(row(DOCUMENT, None), HashMap::from([(9, stored)])).unwrap();

        let attachment = &disclosure.attachments[0];
        assert_eq!(attachment.state, attachment::State::Stored as i32);
        assert_eq!(attachment.size_bytes, 84_743);
        assert_eq!(attachment.name, "q2.pdf");
    }

    #[test]
    fn decode_row__document_not_json__returns_error_naming_the_message() {
        let error = decode_row(row("not json", None), HashMap::new()).unwrap_err();

        assert_eq!(
            error.to_string(),
            "message 7: the stored document does not decode"
        );
    }
}
