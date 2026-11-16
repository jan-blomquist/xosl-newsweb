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
    document                  TEXT    NOT NULL
) STRICT;

CREATE INDEX disclosures_by_time
    ON disclosures (published_at_ns DESC, message_id DESC);
CREATE INDEX disclosures_by_issuer_sign
    ON disclosures (issuer_sign, published_at_ns DESC, message_id DESC);
CREATE INDEX disclosures_by_corrected
    ON disclosures (correction_for_message_id)
    WHERE correction_for_message_id != 0;

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
    PRIMARY KEY (message_id, attachment_id)
) STRICT, WITHOUT ROWID;

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
