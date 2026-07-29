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

/// Execute a shell command and return structured results.
pub async fn execute(command: &str) -> Result<String, ShellError> {
    let output = Command::new("bash").arg("-c").arg(command).output().await?;

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

#[cfg(test)]
mod tests {
    use crate::{ShellError, execute};

    #[tokio::test]
    async fn test_execute_echo() {
        let result = execute("echo hello").await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_execute_failure() {
        let result = execute("false").await;
        if let Err(ShellError::CommandFailed(exit_code, _stderr, _stdout)) = &result {
            assert_eq!(*exit_code, 1);
        } else if let Ok(stdout) = &result {
            panic!("Expected command 'false' to fail, got stdout: {:?}", stdout);
        } else {
            panic!("Unexpected error variant: {:?}", result);
        }
    }
}
