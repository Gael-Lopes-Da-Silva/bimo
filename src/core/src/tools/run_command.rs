use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use std::process::Command;
use std::time::Duration;
use tracing::info;

/// Executes a shell command in the workspace and captures its output.
///
/// Defaults the working directory to the current directory and the timeout to
/// 120 seconds.  Returns both stdout and stderr on success or failure.
#[tool(
    name = "run_command",
    desc = "Execute a shell command in the workspace. Provide the command string and optionally a working directory (defaults to cwd) and timeout in seconds (defaults to 120). Use && to chain commands. Returns stdout and stderr."
)]
pub fn run_command(command: String, workdir: Option<String>, timeout_secs: Option<u64>) -> Tool {
    info!("Running command: {}", command);
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(120));

    let cwd = if let Some(ref dir) = workdir {
        if std::path::Path::new(dir).exists() && std::path::Path::new(dir).is_dir() {
            dir.as_str()
        } else {
            return Err(format!("Invalid working directory: {}", dir));
        }
    } else {
        "."
    };

    let child = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let output = wait_with_timeout(child, timeout, &command).map_err(|e| e.to_string())?;

    let mut result = String::new();

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            result.push_str(&format!("```\n{}\n```\n", stdout.trim_end()));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            result.push_str(&format!("stderr:\n```\n{}\n```\n", stderr.trim_end()));
        }
        if stdout.is_empty() && stderr.is_empty() {
            result.push_str("Command completed successfully (no output).");
        }
        info!("Command succeeded: {}", command);
        Ok(result)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        result.push_str(&format!("Exit code: {:?}\n", output.status.code()));
        if !stdout.is_empty() {
            result.push_str(&format!("stdout:\n```\n{}\n```\n", stdout.trim_end()));
        }
        if !stderr.is_empty() {
            result.push_str(&format!("stderr:\n```\n{}\n```\n", stderr.trim_end()));
        }
        info!(
            "Command failed (exit={:?}): {}",
            output.status.code(),
            command
        );
        Err(result)
    }
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
    command: &str,
) -> crate::Result<std::process::Output> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map_or_else(Vec::new, |mut out| {
                    let mut buf = Vec::new();
                    use std::io::Read;
                    out.read_to_end(&mut buf).unwrap_or_default();
                    buf
                });
                let stderr = child.stderr.take().map_or_else(Vec::new, |mut out| {
                    let mut buf = Vec::new();
                    use std::io::Read;
                    out.read_to_end(&mut buf).unwrap_or_default();
                    buf
                });
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return Err(crate::error::BimoError::Msg(format!(
                        "Command timed out after {}s: {}",
                        timeout.as_secs(),
                        command
                    )));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(crate::error::BimoError::Io(e)),
        }
    }
}
