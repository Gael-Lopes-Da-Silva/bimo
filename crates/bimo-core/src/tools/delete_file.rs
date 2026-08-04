use std::fs;

use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use tracing::info;

/// Deletes a file.
#[tool(
    name = "delete_file",
    desc = "Delete a file. Provide the file_path of the file to delete. Fails if the file does not exist."
)]
pub fn delete_file(file_path: String) -> Tool {
    info!("Deleting file: {}", file_path);
    fs::remove_file(&file_path).map_err(|e| format!("Failed to delete {}: {}", file_path, e))?;
    Ok(format!("Deleted {}", file_path))
}
