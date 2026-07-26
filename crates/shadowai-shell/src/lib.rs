/// Result of executing a shell command, capturing full output and exit status.
#[derive(Debug)]
pub struct ShellResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl std::fmt::Display for ShellResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.success {
            write!(f, "OK")?;
        } else {
            write!(f, "FAIL")?;
        }
        Ok(())
    }
}

impl std::error::Error for ShellResult {}

/// Execute a shell command and return structured results.
pub fn execute(command: &str) -> Result<ShellResult, anyhow::Error> {
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(command)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let success = output.status.success();

    Ok(ShellResult {
        stdout,
        stderr,
        success,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_echo() {
        let result = execute("echo hello").unwrap();
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_execute_failure() {
        let result = execute("false").unwrap();
        assert!(!result.success);
    }
}
