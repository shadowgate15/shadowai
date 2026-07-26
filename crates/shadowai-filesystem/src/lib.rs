use std::path::{Path, PathBuf};

/// Read the contents of a file.
pub async fn read_file(path: PathBuf) -> Result<String, anyhow::Error> {
    let mut file = tokio::fs::File::open(&path).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    Ok(contents)
}

/// Write content to a file, creating parent directories if needed.
pub async fn write_file(path: PathBuf, content: &str) -> Result<(), anyhow::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    let mut file = tokio::fs::File::create(&path).await?;
    file.write_all(content.as_bytes()).await?;
    Ok(())
}

/// Replace occurrences of `old_text` with `new_text` in a file.
/// If the file does not exist, it is created fresh with `new_text`.
pub async fn edit_file(
    path: PathBuf,
    old_text: &str,
    new_text: &str,
) -> Result<String, anyhow::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    // Try to open the existing file first.
    match tokio::fs::File::open(&path).await {
        Ok(mut f) => {
            let mut contents = String::new();
            f.read_to_string(&mut contents).await?;

            let modified = contents.replace(old_text, new_text);

            if modified == contents {
                return Err(anyhow::anyhow!(
                    "No occurrences of '{old_text}' found in the file."
                ));
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
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

