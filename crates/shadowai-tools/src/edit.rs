use std::path::PathBuf;

use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shadowai_filesystem::{EditFileError, edit_file};

#[derive(Deserialize)]
pub struct EditArgs {
    pub file: PathBuf,
    pub old_text: Option<String>,
    pub new_text: String,
}

#[derive(Deserialize, Serialize)]
pub struct EditTool;

impl EditTool {
    pub const DESCRIPTION: &'static str = "Edits the contents of a file at the given path.";
}

impl Tool for EditTool {
    const NAME: &'static str = "edit";

    type Error = EditFileError;
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
        edit_file(args.file, args.old_text.as_deref(), &args.new_text).await
    }
}
