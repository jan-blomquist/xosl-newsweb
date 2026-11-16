//! Integration tests of the attachments' bytes: they run `Files` against an
//! object store. Most use `memory:///`; one writes to a temporary folder
//! through a `file` URL.

#![allow(non_snake_case)]

use bytes::Bytes;
use xosl_newsweb::files::{Files, FilesError};

/// The start of a PDF file; invented content.
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /Catalog >>\nendobj\n";

fn pdf() -> Bytes {
    Bytes::from_static(PDF)
}

fn in_memory() -> Files {
    Files::open("memory:///", []).unwrap()
}

#[tokio::test]
async fn get__after_put__returns_the_bytes_unchanged() {
    let files = in_memory();
    files.put(700001, 340001, pdf()).await.unwrap();

    let bytes = files.get(700001, 340001).await.unwrap();

    assert_eq!(bytes, PDF);
}

#[tokio::test]
async fn put__pdf__describes_it_as_a_pdf_of_its_size() {
    let files = in_memory();

    let file = files.put(700001, 340001, pdf()).await.unwrap();

    assert_eq!(
        (file.content_type.as_str(), file.size_bytes),
        ("application/pdf", PDF.len() as i64)
    );
}

#[tokio::test]
async fn put__same_attachment_again__replaces_the_bytes() {
    let files = in_memory();
    files.put(700001, 340001, pdf()).await.unwrap();

    files
        .put(700001, 340001, Bytes::from_static(b"replacement"))
        .await
        .unwrap();

    assert_eq!(files.get(700001, 340001).await.unwrap(), "replacement");
}

#[tokio::test]
async fn get__other_attachment_of_the_same_message__returns_not_found() {
    let files = in_memory();
    files.put(700001, 340001, pdf()).await.unwrap();

    let error = files.get(700001, 340002).await.unwrap_err();

    assert!(matches!(
        error,
        FilesError::NotFound {
            message_id: 700001,
            attachment_id: 340002
        }
    ));
}

#[tokio::test]
async fn open__file_url_of_missing_folder__reads_back_what_an_earlier_value_wrote() {
    let parent = tempfile::tempdir().unwrap();
    let folder = parent.path().join("mirror").join("attachments");
    let url = url::Url::from_directory_path(&folder).unwrap().to_string();
    let earlier = Files::open(&url, []).unwrap();
    earlier.put(700001, 340001, pdf()).await.unwrap();
    drop(earlier);

    let files = Files::open(&url, []).unwrap();

    assert_eq!(files.get(700001, 340001).await.unwrap(), PDF);
}

#[tokio::test]
async fn open__file_url__writes_under_the_folder_it_names() {
    let parent = tempfile::tempdir().unwrap();
    let url = url::Url::from_directory_path(parent.path())
        .unwrap()
        .to_string();
    let files = Files::open(&url, []).unwrap();

    files.put(700001, 340001, pdf()).await.unwrap();

    assert!(parent.path().join("700001").join("340001").is_file());
}

#[test]
fn open__text_that_is_not_a_url__returns_error_naming_it() {
    let error = Files::open("attachments", []).unwrap_err();

    assert_eq!(error.to_string(), r#""attachments" is not a URL"#);
}

#[test]
fn open__scheme_without_an_enabled_store__returns_error_naming_the_url() {
    let cases = ["gs://bucket/attachments", "ftp://host/attachments"];
    for url in cases {
        assert!(
            matches!(Files::open(url, []), Err(FilesError::Open { url: named, .. }) if named == url),
            "url: {url}"
        );
    }
}
