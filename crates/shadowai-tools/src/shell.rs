use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("Command failed (exit code {0}): {1}\n{2}")]
    CommandFailed(i32, String, String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

#[derive(Deserialize, Serialize)]
pub struct ShellTool;

impl ShellTool {
    pub const DESCRIPTION: &'static str = "Executes a shell command and returns its output.";
}

impl Tool for ShellTool {
    const NAME: &'static str = "shell";

    type Error = ShellError;
    type Args = String;
    type Output = String;

    fn description(&self) -> String {
        ShellTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "description": "The shell command to execute.",
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let output = Command::new("bash").arg("-c").arg(&args).output().await?;

        if !output.status.success() {
            return Err(ShellError::CommandFailed(
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stderr).to_string(),
                String::from_utf8_lossy(&output.stdout).to_string(),
            ));
        }

        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout.trim().to_string())
    }
}
