//! The mirror's database: disclosures, the state of their attachments, and
//! the event log. SQLite, in one file.
//!
//! A disclosure is kept as the exchange's JSON for it and decoded on every
//! read, so a fix to the decoder applies to what is already stored. Every
//! change to a disclosure and the event that reports it are written in one
//! transaction.
//!
//! The store reads no clock: callers pass the current time.

use std::collections::HashMap;
use std::path::Path;

use prost::Message;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{SqliteConnection, SqlitePool};

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
}

impl Store {
    /// Opens the database file, creating it and its tables when missing.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        Self::migrated(pool).await
    }

    /// A database that lives as long as the store. One connection, since
    /// every connection to `:memory:` opens a database of its own.
    pub async fn open_in_memory() -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::migrated(pool).await
    }

    async fn migrated(pool: SqlitePool) -> Result<Self, StoreError> {
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Stores a row of the exchange's list. A disclosure already stored is
    /// left as it is: a list row holds nothing the store does not have.
    pub async fn store_list_entry(&self, entry: &Fetched, now_ns: i64) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = insert_disclosure(&mut tx, entry, false, now_ns).await?;
        if inserted {
            write_corrected_event_for_original(&mut tx, &entry.disclosure, now_ns).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Stores a disclosure read in full, and writes the event for what
    /// changed: `DISCOVERED` on the first full read, `REVISED` when the
    /// exchange's document differs from the stored one, nothing when it is
    /// the same.
    pub async fn store_message(&self, message: &Fetched, now_ns: i64) -> Result<(), StoreError> {
        let message_id = message.disclosure.message_id;
        let mut tx = self.pool.begin().await?;

        let stored: Option<StoredDocument> =
            sqlx::query_as("SELECT read_in_full, document FROM disclosures WHERE message_id = ?")
                .bind(message_id)
                .fetch_optional(&mut *tx)
                .await?;
        let is_new = stored.is_none();
        let change = full_read(stored.as_ref(), &message.document);
        if change == FullRead::Unchanged {
            return Ok(());
        }

        if is_new {
            insert_disclosure(&mut tx, message, true, now_ns).await?;
        } else {
            update_disclosure(&mut tx, message).await?;
        }
        insert_attachments(&mut tx, &message.disclosure).await?;
        if change == FullRead::Revised {
            write_event(&mut tx, event::Kind::Revised, message_id, 0, now_ns).await?;
        } else {
            write_event(&mut tx, event::Kind::Discovered, message_id, 0, now_ns).await?;
            if is_new {
                write_corrected_event_for_original(&mut tx, &message.disclosure, now_ns).await?;
            }
            write_corrected_event_if_corrected(&mut tx, message_id, now_ns).await?;
        }

        tx.commit().await?;
        Ok(())
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
        let mut tx = self.pool.begin().await?;
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
        write_event(&mut tx, kind, message_id, attachment_id, now_ns).await?;
        tx.commit().await?;
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
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE attachments SET state = ? WHERE message_id = ? AND attachment_id = ?",
        )
        .bind(attachment::State::Failed as i32)
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
        let kind = event::Kind::AttachmentFailed;
        write_event(&mut tx, kind, message_id, attachment_id, now_ns).await?;
        tx.commit().await?;
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
            "SELECT message_id, document, read_in_full, received_at_ns,
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
        let seq = sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM events")
            .fetch_one(&self.pool)
            .await?;
        Ok(seq)
    }
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

/// What `store_message` compares an incoming message with.
#[derive(Debug, sqlx::FromRow)]
struct StoredDocument {
    read_in_full: bool,
    document: String,
}

/// A row of `disclosures` as the reads select it.
#[derive(Debug, sqlx::FromRow)]
struct DisclosureRow {
    message_id: i64,
    document: String,
    read_in_full: bool,
    received_at_ns: i64,
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

/// Writes an event that carries the disclosure as it now stands.
async fn write_event(
    connection: &mut SqliteConnection,
    kind: event::Kind,
    message_id: i64,
    attachment_id: i64,
    now_ns: i64,
) -> Result<(), StoreError> {
    let Some(disclosure) = load_disclosure(connection, message_id).await? else {
        return Ok(());
    };
    sqlx::query(
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
    Ok(())
}

/// A correction was stored for the first time: writes `CORRECTED` for the
/// disclosure it corrects, when that one has been read in full. When it has
/// not, `write_corrected_event_if_corrected` writes the event at its full
/// read, so each pair gets one event whichever of the two arrives first.
async fn write_corrected_event_for_original(
    connection: &mut SqliteConnection,
    correction: &Disclosure,
    now_ns: i64,
) -> Result<(), StoreError> {
    let original = correction.correction_for_message_id;
    if original == 0 {
        return Ok(());
    }
    let read_in_full: Option<bool> =
        sqlx::query_scalar("SELECT read_in_full FROM disclosures WHERE message_id = ?")
            .bind(original)
            .fetch_optional(&mut *connection)
            .await?;
    if read_in_full == Some(true) {
        write_event(connection, event::Kind::Corrected, original, 0, now_ns).await?;
    }
    Ok(())
}

/// A disclosure was read in full for the first time: writes `CORRECTED` for
/// it when a stored disclosure corrects it.
async fn write_corrected_event_if_corrected(
    connection: &mut SqliteConnection,
    message_id: i64,
    now_ns: i64,
) -> Result<(), StoreError> {
    let corrected: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM disclosures WHERE correction_for_message_id = ?)",
    )
    .bind(message_id)
    .fetch_one(&mut *connection)
    .await?;
    if corrected {
        write_event(connection, event::Kind::Corrected, message_id, 0, now_ns).await?;
    }
    Ok(())
}

async fn load_disclosure(
    connection: &mut SqliteConnection,
    message_id: i64,
) -> Result<Option<Disclosure>, StoreError> {
    let row: Option<DisclosureRow> = sqlx::query_as(
        "SELECT message_id, document, read_in_full, received_at_ns,
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
