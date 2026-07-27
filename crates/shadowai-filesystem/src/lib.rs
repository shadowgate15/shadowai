use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

/// Read the contents of a file.
pub async fn read_file(path: PathBuf) -> Result<String, std::io::Error> {
    let mut file = tokio::fs::File::open(&path).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    Ok(contents)
}

/// Write content to a file, creating parent directories if needed.
pub async fn write_file(path: PathBuf, content: &str) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    let mut file = tokio::fs::File::create(&path).await?;
    file.write_all(content.as_bytes()).await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum EditFileError {
    #[error("old_text is required when editing an existing file")]
    OldTextNotFound,
    #[error("No occurrences of '{0}' found in the file.")]
    NoOccurrencesFound(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

/// Replace occurrences of `old_text` with `new_text` in a file.
/// If the file does not exist, it is created fresh with `new_text`.
pub async fn edit_file(
    path: PathBuf,
    old_text: Option<&str>,
    new_text: &str,
) -> Result<String, EditFileError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    // Try to open the existing file first.
    match tokio::fs::File::open(&path).await {
        Ok(mut f) => {
            let old_text = old_text.ok_or(EditFileError::OldTextNotFound)?;

            let mut contents = String::new();
            f.read_to_string(&mut contents).await?;

            let modified = contents.replace(old_text, new_text);

            if modified == contents {
                return Err(EditFileError::NoOccurrencesFound(old_text.to_string()));
            }

            let mut out = tokio::fs::File::create(&path).await?;
            out.write_all(modified.as_bytes()).await?;
            Ok(modified)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File does not exist — create it fresh with new_text.
            let mut f = tokio::fs::File::create(&path).await?;
            f.write_all(new_text.as_bytes()).await?;
            Ok(new_text.to_string())
        }
        Err(e) => Err(e.into()),
    }
}
