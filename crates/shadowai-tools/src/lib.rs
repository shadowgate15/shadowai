use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

/// Read a file from the filesystem.
#[rig::tool_macro(description = "Read a file from the filesystem", required(file))]
pub async fn read_file(file: PathBuf) -> Result<String, rig::tool::ToolError> {
    _read_file(file)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _read_file(file: PathBuf) -> Result<String, anyhow::Error> {
    let mut file = tokio::fs::File::open(&file).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    Ok(contents)
}

/// List files matching a glob pattern (e.g., *.rs, src/**/*).
#[rig::tool_macro(
    description = "List files matching a glob pattern (e.g., *.rs, src/**/*)",
    required(pattern)
)]
pub async fn glob_files(pattern: String) -> Result<Vec<String>, rig::tool::ToolError> {
    _glob_files(pattern)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _glob_files(pattern: String) -> Result<Vec<String>, anyhow::Error> {
    let mut results = Vec::new();
    for entry in glob::glob(&pattern)? {
        let path = entry?;
        if path.is_file() {
            results.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(results)
}

/// Replace occurrences of old_text with new_text in a file.
#[rig::tool_macro(
    description = "Replace occurrences of old_text with new_text in a file (creates the file if it doesn't exist).",
    required(file),
    required(old_text),
    required(new_text)
)]
pub async fn edit_file(
    file: PathBuf,
    old_text: String,
    new_text: String,
) -> Result<String, rig::tool::ToolError> {
    _edit_file(file, old_text, new_text)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _edit_file(
    file: PathBuf,
    old_text: String,
    new_text: String,
) -> Result<String, anyhow::Error> {
    let parent = file.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    // Try to open existing file first
    match tokio::fs::File::open(&file).await {
        Ok(mut f) => {
            let mut contents = String::new();
            f.read_to_string(&mut contents).await?;

            let modified = contents.replace(old_text.as_str(), new_text.as_str());

            if modified == contents {
                return Err(anyhow::anyhow!(
                    "No occurrences of '{old_text}' found in the file."
                ));
            }

            let mut out = tokio::fs::File::create(&file).await?;
            out.write_all(modified.as_bytes()).await?;
            Ok(modified)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist — create it fresh with new_text
            let mut f = tokio::fs::File::create(&file).await?;
            f.write_all(new_text.as_bytes()).await?;
            Ok(new_text)
        }
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}

/// Execute a bash/shell command on the host machine and return its stdout output.
#[rig::tool_macro(
    description = "Execute a bash/shell command on the host machine and return its stdout.",
    required(command)
)]
pub async fn shell_command(command: String) -> Result<String, rig::tool::ToolError> {
    _shell_command(command)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _shell_command(command: String) -> Result<String, anyhow::Error> {
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(&command)
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Command failed (exit code {}): {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout.trim().to_string())
}

/// Repair hook that handles invalid tool calls.
pub struct RepairToolCall;

impl<M: rig::prelude::CompletionModel> rig::agent::AgentHook<M> for RepairToolCall {
    async fn on_event(
        &self,
        _ctx: &rig::agent::HookContext,
        event: rig::agent::StepEvent<'_, M>,
    ) -> rig::agent::Flow {
        match event {
            rig::agent::StepEvent::InvalidToolCall(ctx) if ctx.tool_name == "write_file" => {
                rig::agent::Flow::repair("edit_file")
            }
            rig::agent::StepEvent::InvalidToolCall(ctx) => rig::agent::Flow::retry(format!(
                "Use one of these tools: {:?}",
                ctx.available_tools
            )),
            _ => rig::agent::Flow::cont(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join("shadowai_tools_test.tmp");
        tokio::fs::write(&tmp, "hello world").await?;
        let result = read_file(tmp).await?;
        assert_eq!(result.trim(), "hello world");
        Ok(())
    }

    #[tokio::test]
    async fn test_edit_file() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join("shadowai_tools_test2.tmp");
        tokio::fs::write(&tmp, "old text").await?;
        let result = edit_file(tmp, "old text".to_string(), "new text".to_string()).await?;
        assert_eq!(result.trim(), "new text");
        Ok(())
    }

    #[tokio::test]
    async fn test_shell_command() -> anyhow::Result<()> {
        let result = shell_command("echo hello_from_tools".to_string()).await?;
        assert_eq!(result, "hello_from_tools");
        Ok(())
    }
}
