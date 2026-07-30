use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use std::fs;
use std::path::Path;
use tracing::info;

/// Writes content to a file, creating or overwriting it.
///
/// Creates parent directories if they do not exist.
#[tool(
    name = "write_file",
    desc = "Write content to a file, creating or overwriting it. Provide the file_path and the content to write. Creates parent directories if needed."
)]
pub fn write_file(file_path: String, content: String) -> Tool {
    info!("Writing file: {}", file_path);
    let path = Path::new(&file_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create parent directories for {}: {}",
                file_path, e
            )
        })?;
    }

    fs::write(path, &content).map_err(|e| format!("Failed to write {}: {}", file_path, e))?;

    let line_count = content.lines().count();
    Ok(format!("Written {} lines to {}", line_count, file_path))
}
