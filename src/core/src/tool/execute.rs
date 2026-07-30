use std::path::Path;

use tokio::time::Duration;

use super::ParsedToolCall;

pub type ToolResult = std::result::Result<String, String>;

pub async fn execute_tool(call: &ParsedToolCall) -> ToolResult {
    match call.name.as_str() {
        "read_file" => execute_read_file(call).await,
        "edit_file" => execute_edit_file(call).await,
        "write_file" => execute_write_file(call).await,
        "run_command" => execute_run_command(call).await,
        "manage_todo" => Ok("Todo management not implemented in core, handle at client level".into()),
        _ => Err(format!("Unknown tool: {}", call.name)),
    }
}

async fn execute_read_file(call: &ParsedToolCall) -> ToolResult {
    let path = call
        .get_arg_string("path")
        .ok_or_else(|| "Missing required 'path' argument".to_string())?;

    let contents = tokio::fs::read_to_string(Path::new(&path))
        .await
        .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

    let line_count = contents.lines().count();
    Ok(format!(
        "File '{}' ({lines} lines):\n{content}",
        path,
        lines = line_count,
        content = contents
    ))
}

async fn execute_edit_file(call: &ParsedToolCall) -> ToolResult {
    let path = call
        .get_arg_string("path")
        .ok_or_else(|| "Missing required 'path' argument".to_string())?;
    let old_string = call
        .get_arg_string("old_string")
        .ok_or_else(|| "Missing required 'old_string' argument".to_string())?;
    let new_string = call
        .get_arg_string("new_string")
        .ok_or_else(|| "Missing required 'new_string' argument".to_string())?;

    let content = tokio::fs::read_to_string(Path::new(&path))
        .await
        .map_err(|e| format!("Failed to read '{}': {}", path, e))?;

    if !content.contains(&old_string) {
        return Err(format!(
            "old_string not found in '{}'. The exact text to replace must exist in the file.",
            path
        ));
    }

    let new_content = content.replace(&old_string, &new_string);
    if new_content == content {
        return Err("No changes made - old_string and new_string are identical.".to_string());
    }

    tokio::fs::write(Path::new(&path), &new_content)
        .await
        .map_err(|e| format!("Failed to write '{}': {}", path, e))?;

    let diff = count_diff_lines(&content, &new_content);
    Ok(format!(
        "Edited '{}' ({} line(s) changed).",
        path, diff
    ))
}

async fn execute_write_file(call: &ParsedToolCall) -> ToolResult {
    let path = call
        .get_arg_string("path")
        .ok_or_else(|| "Missing required 'path' argument".to_string())?;
    let content = call
        .get_arg_string("content")
        .ok_or_else(|| "Missing required 'content' argument".to_string())?;

    if let Some(parent) = Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create parent directories for '{}': {}", path, e))?;
    }

    tokio::fs::write(Path::new(&path), &content)
        .await
        .map_err(|e| format!("Failed to write '{}': {}", path, e))?;

    let line_count = content.lines().count();
    Ok(format!(
        "Written {} lines to '{}'.",
        line_count, path
    ))
}

async fn execute_run_command(call: &ParsedToolCall) -> ToolResult {
    let command = call
        .get_arg_string("command")
        .ok_or_else(|| "Missing required 'command' argument".to_string())?;

    let timeout_secs = call.get_arg_number("timeout").unwrap_or(60.0) as u64;
    let timeout = Duration::from_secs(timeout_secs);

    let result = tokio::time::timeout(timeout, run_shell_command(&command)).await;

    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("Command failed: {}", e)),
        Err(_) => Err(format!(
            "Command timed out after {} seconds: {}",
            timeout_secs, command
        )),
    }
}

async fn run_shell_command(cmd: &str) -> std::result::Result<String, String> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let mut result = String::new();

    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        result.push_str("STDOUT:\n");
        result.push_str(&stdout);
        if !result.ends_with('\n') {
            result.push('\n');
        }
    }

    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        result.push_str("STDERR:\n");
        result.push_str(&stderr);
        if !result.ends_with('\n') {
            result.push('\n');
        }
    }

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        result.push_str(&format!("Exit code: {}", exit_code));
    } else if result.is_empty() {
        result.push_str("Command completed successfully (no output).");
    }

    Ok(result)
}

fn count_diff_lines(old: &str, new: &str) -> usize {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let max = old_lines.len().max(new_lines.len());
    let mut changes = 0;
    for i in 0..max {
        let a = old_lines.get(i).copied().unwrap_or("");
        let b = new_lines.get(i).copied().unwrap_or("");
        if a != b {
            changes += 1;
        }
    }
    changes
}
