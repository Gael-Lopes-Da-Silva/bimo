use std::fs;

use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use tracing::info;

/// Reads the contents of a file, optionally restricting to a line range.
///
/// Lines are 1-indexed.  When both `start_line` and `end_line` are `None`
/// the entire file is returned.
#[tool(
    name = "read_file",
    desc = "Read the contents of a file. Optionally specify start_line and end_line (1-indexed) to read a range. Leave both None to read the entire file."
)]
pub fn read_file(file_path: String, start_line: Option<usize>, end_line: Option<usize>) -> Tool {
    info!("Reading file: {}", file_path);
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    match (start_line, end_line) {
        (Some(start), Some(end)) => {
            let start = start.saturating_sub(1);
            let end = end.min(total_lines);
            if start >= total_lines || start >= end {
                return Err(format!(
                    "Invalid range: {}-{} (file has {total_lines} lines)",
                    start + 1,
                    end
                ));
            }
            let selected = lines[start..end].join("\n");
            Ok(format!(
                "```\n{}:{}\n{}\n```\n(Lines {}-{} of {})",
                file_path,
                start + 1,
                selected,
                start + 1,
                end,
                total_lines
            ))
        }
        (Some(start), None) => {
            let start = start.saturating_sub(1);
            if start >= total_lines {
                return Err(format!(
                    "Start line {} exceeds file length ({total_lines})",
                    start + 1
                ));
            }
            let selected = lines[start..].join("\n");
            Ok(format!(
                "```\n{}:{}\n{}\n```\n(Lines {}-{} of {})",
                file_path,
                start + 1,
                selected,
                start + 1,
                total_lines,
                total_lines
            ))
        }
        (None, Some(end)) => {
            let end = end.min(total_lines);
            let selected = lines[..end].join("\n");
            Ok(format!(
                "```\n{}:{}\n{}\n```\n(Lines 1-{} of {})",
                file_path, 1, selected, end, total_lines
            ))
        }
        (None, None) => Ok(format!(
            "```\n{}\n{}\n```\n({} lines)",
            file_path, content, total_lines
        )),
    }
}
