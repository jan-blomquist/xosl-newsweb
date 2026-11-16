//! The bytes of attachments, in an object store named by a URL:
//! `file:///absolute/folder`, or `memory:///` in tests. The key of an
//! attachment is `{message_id}/{attachment_id}` under the path the URL
//! carries.
//!
//! Every read and write goes through the `object_store` crate, so another
//! kind of store is a cargo feature of that crate and a URL, not new code
//! here. Only the local filesystem is compiled in.

use std::sync::Arc;

use bytes::Bytes;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use sha2::{Digest, Sha256};
use url::Url;

use crate::store::StoredFile;

/// The content type of bytes whose type `infer` does not detect.
const UNKNOWN_CONTENT_TYPE: &str = "application/octet-stream";

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("{url:?} is not a URL")]
    Url {
        url: String,
        #[source]
        source: url::ParseError,
    },
    #[error("{url:?} does not name a local folder")]
    NotAFolder { url: String },
    #[error("cannot create the folder of {url:?}")]
    Folder {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot open the object store at {url:?}")]
    Open {
        url: String,
        #[source]
        source: object_store::Error,
    },
    #[error("no file is stored for attachment {attachment_id} of message {message_id}")]
    NotFound { message_id: i64, attachment_id: i64 },
    #[error(transparent)]
    ObjectStore(#[from] object_store::Error),
}

/// The attachments' bytes.
#[derive(Debug, Clone)]
pub struct Files {
    objects: Arc<dyn ObjectStore>,
    /// The path the URL carries; every key is under it.
    prefix: Path,
}

impl Files {
    /// The object store at `url`. `options` are the object store's
    /// configuration keys for the URL's scheme; `file` and `memory` take
    /// none. For a `file` URL, the folder is created when missing.
    pub fn open(
        url: &str,
        options: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, FilesError> {
        let parsed = Url::parse(url).map_err(|source| FilesError::Url {
            url: url.to_owned(),
            source,
        })?;
        if parsed.scheme() == "file" {
            create_folder(&parsed, url)?;
        }
        let (objects, prefix) =
            object_store::parse_url_opts(&parsed, options).map_err(|source| FilesError::Open {
                url: url.to_owned(),
                source,
            })?;
        Ok(Self {
            objects: Arc::from(objects),
            prefix,
        })
    }

    /// Stores an attachment's bytes, replacing any stored before, and
    /// describes them.
    pub async fn put(
        &self,
        message_id: i64,
        attachment_id: i64,
        bytes: Bytes,
    ) -> Result<StoredFile, FilesError> {
        let file = describe(&bytes);
        let key = key(&self.prefix, message_id, attachment_id);
        self.objects.put(&key, PutPayload::from(bytes)).await?;
        Ok(file)
    }

    /// An attachment's bytes.
    pub async fn get(&self, message_id: i64, attachment_id: i64) -> Result<Bytes, FilesError> {
        let key = key(&self.prefix, message_id, attachment_id);
        let result = match self.objects.get(&key).await {
            Ok(result) => result,
            Err(object_store::Error::NotFound { .. }) => {
                return Err(FilesError::NotFound {
                    message_id,
                    attachment_id,
                });
            }
            Err(error) => return Err(error.into()),
        };
        Ok(result.bytes().await?)
    }
}

/// Creates the folder a `file` URL names. `LocalFileSystem` writes the
/// folders below it, but not the folder itself.
fn create_folder(parsed: &Url, url: &str) -> Result<(), FilesError> {
    let folder = parsed.to_file_path().map_err(|()| FilesError::NotAFolder {
        url: url.to_owned(),
    })?;
    std::fs::create_dir_all(folder).map_err(|source| FilesError::Folder {
        url: url.to_owned(),
        source,
    })
}

fn key(prefix: &Path, message_id: i64, attachment_id: i64) -> Path {
    prefix
        .clone()
        .join(message_id.to_string())
        .join(attachment_id.to_string())
}

/// The size, SHA-256 and content type of an attachment's bytes. The content
/// type is detected from the bytes: the exchange serves every attachment as
/// `application/octet-stream`, whatever its name.
pub fn describe(bytes: &[u8]) -> StoredFile {
    let content_type = infer::get(bytes).map_or(UNKNOWN_CONTENT_TYPE, |kind| kind.mime_type());
    StoredFile {
        content_type: content_type.to_owned(),
        size_bytes: i64::try_from(bytes.len()).expect("a slice is shorter than i64::MAX bytes"),
        sha256: hex::encode(Sha256::digest(bytes)),
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// The start of a PDF file; `infer` reads the first bytes only.
    const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n";
    /// The signature and first chunk header of a PNG image.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    /// The SHA-256 of "abc", from FIPS 180-2.
    const SHA256_OF_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn describe__known_formats__detects_the_content_type() {
        let cases: [(&[u8], &str); 4] = [
            (PDF, "application/pdf"),
            (PNG, "image/png"),
            (b"plain words", UNKNOWN_CONTENT_TYPE),
            (b"", UNKNOWN_CONTENT_TYPE),
        ];
        for (bytes, expected) in cases {
            assert_eq!(describe(bytes).content_type, expected, "bytes: {bytes:?}");
        }
    }

    #[test]
    fn describe__abc__returns_lowercase_hex_sha256() {
        let file = describe(b"abc");

        assert_eq!(file.sha256, SHA256_OF_ABC);
    }

    #[test]
    fn describe__abc__returns_size_of_three_bytes() {
        let file = describe(b"abc");

        assert_eq!(file.size_bytes, 3);
    }

    #[test]
    fn key__prefixes__puts_message_then_attachment_under_the_prefix() {
        let cases = [
            ("", "700001/340001"),
            ("attachments", "attachments/700001/340001"),
            ("mirror/attachments", "mirror/attachments/700001/340001"),
        ];
        for (prefix, expected) in cases {
            assert_eq!(
                key(&Path::from(prefix), 700001, 340001).as_ref(),
                expected,
                "prefix: {prefix:?}"
            );
        }
    }
}
