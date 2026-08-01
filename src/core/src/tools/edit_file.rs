use std::fs;

use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use tracing::info;

/// Makes a precise string replacement in an existing file.
///
/// When `replace_all` is `true` all occurrences of `old_string` are replaced;
/// otherwise only the first occurrence is replaced.
#[tool(
    name = "edit_file",
    desc = "Make a precise string replacement in an existing file. Provide the file_path, the old_string to find, and the new_string to replace it with. The old_string must match exactly, including whitespace. Use replace_all=true to replace all occurrences."
)]
pub fn edit_file(
    file_path: String,
    old_string: String,
    new_string: String,
    replace_all: Option<bool>,
) -> Tool {
    info!("Editing file: {}", file_path);

    if old_string.is_empty() {
        return Err("old_string cannot be empty".to_string());
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    if !content.contains(&old_string) {
        return Err(format!(
            "old_string not found in {}.\n\nExpected to find:\n```\n{}\n```",
            file_path, old_string
        ));
    }

    let replace_all = replace_all.unwrap_or(false);
    let new_content = if replace_all {
        content.replace(&old_string, &new_string)
    } else {
        match content.find(&old_string) {
            Some(pos) => {
                let before = &content[..pos];
                let after = &content[pos + old_string.len()..];
                format!("{before}{new_string}{after}")
            }
            // Guaranteed to match: `content.contains(&old_string)` was checked
            // above, so a first occurrence always exists.
            None => unreachable!(),
        }
    };

    if new_content == content {
        return Err("No changes made - old_string and new_string are identical".to_string());
    }

    fs::write(&file_path, &new_content)
        .map_err(|e| format!("Failed to write {}: {}", file_path, e))?;

    let diff_summary = format!(
        "Replaced \"{}\" -> \"{}\" in {}",
        &old_string[..old_string.len().min(50)],
        &new_string[..new_string.len().min(50)],
        file_path
    );
    info!("{}", diff_summary);
    Ok(diff_summary)
}
