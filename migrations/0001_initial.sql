-- One migration until the first release: edited in place until then,
-- append-only afterwards.

-- One row per disclosure. `document` is the exchange's JSON for it, kept as
-- sent; what the mirror serves is decoded from it on every read. The other
-- columns exist to select and order rows, or hold what the exchange does not
-- send.
CREATE TABLE disclosures (
    message_id                INTEGER PRIMARY KEY,
    published_at_ns           INTEGER NOT NULL,
    -- When the mirror first stored the disclosure.
    received_at_ns            INTEGER NOT NULL,
    issuer_sign               TEXT    NOT NULL,
    is_test                   INTEGER NOT NULL,
    -- 0 when the disclosure corrects none.
    correction_for_message_id INTEGER NOT NULL,
    -- 0: `document` is a row of the exchange's list. 1: it is the full message.
    read_in_full              INTEGER NOT NULL,
    document                  TEXT    NOT NULL,
    -- 1: the exchange's list shows the disclosure changed after it was read in
    -- full, so it is read in full again. The full read sets it back to 0.
    reread                    INTEGER NOT NULL DEFAULT 0,
    -- 1: the full read failed permanently `MAX_PERMANENT_FAILURES` times and
    -- is not requested again.
    failed                    INTEGER NOT NULL DEFAULT 0,
    -- Permanent failures of the full read since its last success.
    attempts                  INTEGER NOT NULL DEFAULT 0,
    -- Consecutive transient failures of the full read; they set the delay
    -- before the next try. 0 after a success or a permanent failure.
    transient_failures        INTEGER NOT NULL DEFAULT 0,
    -- The full read is not tried before this time.
    retry_after_ns            INTEGER NOT NULL DEFAULT 0,
    -- The latest failure of the full read; '' after a success.
    last_error                TEXT    NOT NULL DEFAULT '',
    -- When the latest failed full read was tried.
    last_attempt_at_ns        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX disclosures_by_time
    ON disclosures (published_at_ns DESC, message_id DESC);
CREATE INDEX disclosures_by_issuer_sign
    ON disclosures (issuer_sign, published_at_ns DESC, message_id DESC);
CREATE INDEX disclosures_by_corrected
    ON disclosures (correction_for_message_id)
    WHERE correction_for_message_id != 0;
-- The queue of disclosures to read in full. The query repeats this WHERE
-- clause literally, so that SQLite can use the index.
CREATE INDEX disclosures_unread
    ON disclosures (published_at_ns DESC, message_id DESC)
    WHERE (read_in_full = 0 OR reread = 1) AND failed = 0;

-- One row per attachment of a disclosure read in full. Its name is in the
-- disclosure's document; this table holds the state of its bytes.
CREATE TABLE attachments (
    message_id    INTEGER NOT NULL REFERENCES disclosures (message_id),
    attachment_id INTEGER NOT NULL,
    -- Attachment.State of the contract.
    state         INTEGER NOT NULL,
    content_type  TEXT    NOT NULL DEFAULT '',
    size_bytes    INTEGER NOT NULL DEFAULT 0,
    sha256        TEXT    NOT NULL DEFAULT '',
    -- The columns below have the meaning of those of `disclosures`, for the
    -- download of the bytes. When `attempts` reaches the maximum, `state`
    -- becomes FAILED.
    attempts           INTEGER NOT NULL DEFAULT 0,
    transient_failures INTEGER NOT NULL DEFAULT 0,
    retry_after_ns     INTEGER NOT NULL DEFAULT 0,
    last_error         TEXT    NOT NULL DEFAULT '',
    last_attempt_at_ns INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (message_id, attachment_id)
) STRICT, WITHOUT ROWID;

-- The queue of attachments to download. 1 is Attachment.State PENDING; the
-- query repeats the literal, so that SQLite can use the index.
CREATE INDEX attachments_pending
    ON attachments (message_id, attachment_id)
    WHERE state = 1;

-- The log that WatchEvents serves. Append-only; `seq` is never reused.
CREATE TABLE events (
    seq           INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Event.Kind of the contract.
    kind          INTEGER NOT NULL,
    at_ns         INTEGER NOT NULL,
    message_id    INTEGER NOT NULL,
    -- 0 unless the event is about an attachment.
    attachment_id INTEGER NOT NULL,
    -- The Disclosure as it stood when the event was written, encoded as the
    -- contract's protobuf message.
    disclosure    BLOB    NOT NULL
) STRICT;

-- One row per failed attempt to list a day, read a disclosure in full or
-- download an attachment. Append-only; a success writes no row.
CREATE TABLE failures (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    at_ns         INTEGER NOT NULL,
    -- 1: list a day. 2: read a disclosure in full. 3: download an attachment.
    stage         INTEGER NOT NULL,
    -- 1: a retry cannot change the result, such as a 404.
    permanent     INTEGER NOT NULL,
    -- 0 when the stage is 1.
    message_id    INTEGER NOT NULL,
    -- 0 unless the stage is 3.
    attachment_id INTEGER NOT NULL,
    -- `YYYY-MM-DD`, the Oslo calendar day, when the stage is 1; '' otherwise.
    day           TEXT    NOT NULL,
    detail        TEXT    NOT NULL
) STRICT;

-- One row per Oslo calendar day that the backfill has listed. Listing a day
-- again replaces its row.
CREATE TABLE listed_days (
    -- `YYYY-MM-DD`.
    day          TEXT    PRIMARY KEY,
    listed_at_ns INTEGER NOT NULL,
    -- Rows on the list, `undecodable` included.
    rows         INTEGER NOT NULL,
    -- 1: the exchange truncated the list.
    overflow     INTEGER NOT NULL,
    -- Rows on the list that did not decode.
    undecodable  INTEGER NOT NULL,
    -- 1: the day is known to be complete.
    sealed       INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- One row per consumer of the event log that the mirror delivers to, such as
-- the webhook: the `seq` of the last event delivered to it.
CREATE TABLE delivery_positions (
    name TEXT    PRIMARY KEY,
    seq  INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
