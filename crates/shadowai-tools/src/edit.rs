use std::path::PathBuf;

use rig::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shadowai_filesystem::{EditFileError, edit_file};

#[derive(Deserialize, JsonSchema)]
pub struct EditArgs {
    /// The path to the file to edit.
    pub file: PathBuf,
    /// The text to replace in the file. Required if the file already exists.
    pub old_text: Option<String>,
    /// The new text to write to the file.
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
        schemars::schema_for!(EditArgs).to_value()
    }

    async fn call(
        &self,
        _ctx: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        edit_file(args.file, args.old_text.as_deref(), &args.new_text).await
    }
}
