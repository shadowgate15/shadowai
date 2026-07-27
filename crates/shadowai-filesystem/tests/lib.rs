use std::io::ErrorKind;
use test_dir::{DirBuilder, TestDir};

use shadowai_filesystem::edit_file;
use shadowai_filesystem::read_file;
use shadowai_filesystem::write_file;

#[tokio::test]
async fn read_file_success() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("hello.txt");
    write_file(file_path.clone(), "Hello, world!")
        .await
        .unwrap();
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents, "Hello, world!");
}

#[tokio::test]
async fn read_file_nonexistent() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("does_not_exist.txt");
    let result = read_file(file_path).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn read_file_empty() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("empty.txt");
    write_file(file_path.clone(), "").await.unwrap();
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents, "");
}

#[tokio::test]
async fn read_large_content() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("large.txt");
    let large = "x".repeat(1_048_576);
    write_file(file_path.clone(), &large).await.unwrap();
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents.len(), 1_048_576);
}

#[tokio::test]
async fn write_file_basic() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("basic.txt");
    write_file(file_path.clone(), "content").await.unwrap();
    assert!(file_path.exists());
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents, "content");
}

#[tokio::test]
async fn write_creates_parent_dirs() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("nested/deep/path/file.txt");
    write_file(file_path.clone(), "deep content").await.unwrap();
    assert!(file_path.exists());
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents, "deep content");
}

#[tokio::test]
async fn write_overwrites_existing() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("overwrite.txt");
    write_file(file_path.clone(), "first").await.unwrap();
    write_file(file_path.clone(), "second").await.unwrap();
    let contents = read_file(file_path).await.unwrap();
    assert_eq!(contents, "second");
}

#[tokio::test]
async fn write_multiline() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("multiline.txt");
    let content = "line1\nline2\nline3";
    write_file(file_path.clone(), content).await.unwrap();
    let read_back = read_file(file_path).await.unwrap();
    assert_eq!(read_back, content);
}

#[tokio::test]
async fn edit_replace_existing() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("edit.txt");
    write_file(file_path.clone(), "Hello World").await.unwrap();
    let result = edit_file(file_path.clone(), Some("World"), "Tokio")
        .await
        .unwrap();
    assert_eq!(result, "Hello Tokio");
}

#[tokio::test]
async fn edit_no_old_text() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("edit.txt");
    write_file(file_path.clone(), "Hello World").await.unwrap();
    let result = edit_file(file_path.clone(), None, "Tokio").await;
    assert!(matches!(
        result,
        Err(shadowai_filesystem::EditFileError::OldTextNotFound)
    ));
}

#[tokio::test]
async fn edit_no_occurrences() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("edit.txt");
    write_file(file_path.clone(), "Hello World").await.unwrap();
    let result = edit_file(file_path.clone(), Some("xyz"), "replaced").await;
    assert!(matches!(
        result,
        Err(shadowai_filesystem::EditFileError::NoOccurrencesFound(_))
    ));
}

#[tokio::test]
async fn edit_multiple_occurrences() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("multi.txt");
    write_file(file_path.clone(), "foo bar foo baz")
        .await
        .unwrap();
    let result = edit_file(file_path.clone(), Some("foo"), "qux")
        .await
        .unwrap();
    assert_eq!(result, "qux bar qux baz");
}

#[tokio::test]
async fn edit_creates_new_file() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("new.txt");
    assert!(!file_path.exists());
    let result = edit_file(file_path.clone(), None, "fresh content")
        .await
        .unwrap();
    assert_eq!(result, "fresh content");
    assert!(file_path.exists());
}

#[tokio::test]
async fn edit_creates_parent_dirs() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("nested/new.txt");
    assert!(!dir.join("nested").exists());
    let result = edit_file(file_path.clone(), None, "content").await.unwrap();
    assert_eq!(result, "content");
    assert!(file_path.exists());
}

#[tokio::test]
async fn edit_preserves_other_content() {
    let temp = TestDir::temp();
    let dir = temp.root();

    let file_path = dir.join("preserve.txt");
    write_file(file_path.clone(), "keep this\nreplace that")
        .await
        .unwrap();
    let result = edit_file(file_path.clone(), Some("that"), "other")
        .await
        .unwrap();
    assert_eq!(result, "keep this\nreplace other");
}
