use std::path::{Path, PathBuf};

use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Deserialize)]
pub struct EditArgs {
    pub file: PathBuf,
    pub old_text: Option<String>,
    pub new_text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("old_text is required when editing an existing file")]
    OldTextNotFound,
    #[error("No occurrences of '{0}' found in the file.")]
    NoOccurrencesFound(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

#[derive(Deserialize, Serialize)]
pub struct EditTool;

impl EditTool {
    pub const DESCRIPTION: &'static str = "Edits the contents of a file at the given path.";
}

impl Tool for EditTool {
    const NAME: &'static str = "edit";

    type Error = EditError;
    type Args = EditArgs;
    type Output = String;

    fn description(&self) -> String {
        EditTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "The path to the file to edit."
                },
                "old_text": {
                    "type": "string",
                    "description": "The text to replace in the file. Required if the file already exists."
                },
                "new_text": {
                    "type": "string",
                    "description": "The new text to write to the file."
                }
            },
            "required": ["file", "new_text"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let parent = args.file.parent().unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(parent).await?;

        // Try to open existing file first
        match tokio::fs::File::open(&args.file).await {
            Ok(mut f) => {
                let old_text = args.old_text.ok_or(EditError::OldTextNotFound)?;

                let mut contents = String::new();
                f.read_to_string(&mut contents).await?;

                let modified = contents.replace(old_text.as_str(), args.new_text.as_str());

                if modified == contents {
                    return Err(EditError::NoOccurrencesFound(old_text));
                }

                let mut out = tokio::fs::File::create(&args.file).await?;
                out.write_all(modified.as_bytes()).await?;
                Ok(modified)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist — create it fresh with new_text
                let mut f = tokio::fs::File::create(&args.file).await?;
                f.write_all(args.new_text.as_bytes()).await?;
                Ok(args.new_text)
            }
            Err(e) => Err(e.into()),
        }
    }
}
