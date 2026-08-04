use std::fs;
use std::path::Path;

use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use tracing::info;

/// Creates a new file with the given content.
///
/// Fails if the file already exists. Creates parent directories if they do
/// not exist.
#[tool(
    name = "create_file",
    desc = "Create a new file with the given content. Provide the file_path and the content to write. Fails if the file already exists. Creates parent directories if needed."
)]
pub fn create_file(file_path: String, content: String) -> Tool {
    info!("Creating file: {}", file_path);
    let path = Path::new(&file_path);

    if path.exists() {
        return Err(format!("Cannot create {}: file already exists", file_path));
    }

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
    Ok(format!("Created {} with {} lines", file_path, line_count))
}
