use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::registry::ToolRegistry;

const MAX_OUTPUT_BYTES: usize = 50_000;

static TAG_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<(\w+)\s+([^>]*?)/?>").unwrap());
static ATTR_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(\w+)=(?:"([^"]*)"|'([^']*)')"#).unwrap());

/// A parsed tool call extracted from the LLM's response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub arguments: HashMap<String, String>,
}

/// The result of executing a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub arguments: HashMap<String, String>,
    pub output: String,
    pub is_error: bool,
}

/// Parse all tool calls from an LLM response string.
///
/// Looks for patterns like:
///   <tool_name param1="value1" param2="value2" />
///   <tool_name param1="value1" />
///
/// Also handles content between the tags for tools that use it (e.g. write_file).
pub fn parse_tool_calls(input: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let tag_pattern = &*TAG_PATTERN;
    let attr_pattern = &*ATTR_PATTERN;

    for cap in tag_pattern.captures_iter(input) {
        let name = cap[1].to_string();
        if is_common_tag(&name) {
            continue;
        }

        let attrs_str = &cap[2];
        let mut arguments = HashMap::new();

        for attr_cap in attr_pattern.captures_iter(attrs_str) {
            let key = attr_cap[1].to_string();
            let value = if let Some(v) = attr_cap.get(2) {
                v.as_str().to_string()
            } else if let Some(v) = attr_cap.get(3) {
                v.as_str().to_string()
            } else {
                continue;
            };
            arguments.insert(key, value);
        }

        let full_match = cap.get(0).unwrap();
        let full_tag = full_match.as_str();
        if full_tag.ends_with('>') && !full_tag.ends_with("/>") {
            let close_tag = format!("</{}>", name);
            let search_start = full_match.end();
            if let Some(close_pos) = input[search_start..].find(&close_tag) {
                let content = &input[search_start..search_start + close_pos];
                let content = content.trim().to_string();
                if !content.is_empty() {
                    arguments.insert("content".into(), content);
                }
            }
        }

        calls.push(ToolCall { name, arguments });
    }

    calls
}

fn is_common_tag(name: &str) -> bool {
    matches!(
        name,
        "tool"
            | "name"
            | "description"
            | "parameters"
            | "param"
            | "result"
            | "output"
            | "thinking"
            | "reason"
            | "response"
            | "answer"
            | "code"
            | "pre"
            | "b"
            | "i"
            | "em"
            | "strong"
            | "br"
            | "hr"
            | "div"
            | "span"
            | "p"
            | "ul"
            | "ol"
            | "li"
            | "a"
            | "img"
            | "table"
            | "tr"
            | "td"
            | "th"
    )
}

/// Execute a single tool call and return the result.
pub async fn execute_tool_call(call: &ToolCall, registry: &ToolRegistry) -> ToolResult {
    let name = call.name.clone();
    let arguments = call.arguments.clone();

    let output = match name.as_str() {
        "read_file" => execute_read_file(&arguments),
        "write_file" => execute_write_file(&arguments),
        "list_files" => execute_list_files(&arguments),
        "run_command" => execute_run_command(&arguments).await,
        "search_files" => execute_search_files(&arguments),
        "search_content" => execute_search_content(&arguments),
        _ => {
            if registry.get(&name).is_some() {
                format!("[Tool '{}' is registered but not yet implemented]", name)
            } else {
                format!("[Unknown tool: '{}']", name)
            }
        }
    };

    let is_error = output.starts_with("[Error]") || output.starts_with("[Unknown tool");
    ToolResult {
        name,
        arguments,
        output,
        is_error,
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

fn execute_read_file(args: &HashMap<String, String>) -> String {
    let path = match args.get("path") {
        Some(p) => p,
        None => return "[Error] Missing required parameter: path".into(),
    };

    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => format!("[Error] Failed to read '{}': {}", path, e),
    }
}

fn execute_write_file(args: &HashMap<String, String>) -> String {
    let path = match args.get("path") {
        Some(p) => p,
        None => return "[Error] Missing required parameter: path".into(),
    };
    let content = match args.get("content") {
        Some(c) => c,
        None => return "[Error] Missing required parameter: content".into(),
    };

    if let Some(parent) = Path::new(path).parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return format!("[Error] Failed to create directories for '{}': {}", path, e);
    }

    match std::fs::write(path, content) {
        Ok(()) => format!("Successfully wrote {} bytes to '{}'", content.len(), path),
        Err(e) => format!("[Error] Failed to write '{}': {}", path, e),
    }
}

fn execute_list_files(args: &HashMap<String, String>) -> String {
    let path = args.get("path").map(|s| s.as_str()).unwrap_or(".");

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut items: Vec<String> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                    if is_dir { format!("{}/", name) } else { name }
                })
                .collect();
            items.sort();
            if items.is_empty() {
                "(empty directory)".into()
            } else {
                items.join("\n")
            }
        }
        Err(e) => format!("[Error] Failed to list '{}': {}", path, e),
    }
}

async fn execute_run_command(args: &HashMap<String, String>) -> String {
    let command = match args.get("command") {
        Some(c) => c,
        None => return "[Error] Missing required parameter: command".into(),
    };

    use tokio::time::{Duration, timeout};

    let fut = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output();

    match timeout(Duration::from_secs(30), fut).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push_str("\n--- stderr ---\n");
                }
                result.push_str(&stderr);
            }
            if result.len() > MAX_OUTPUT_BYTES {
                let truncated: String = result.chars().take(MAX_OUTPUT_BYTES).collect();
                format!(
                    "{}\n\n[Output truncated at {} chars]",
                    truncated, MAX_OUTPUT_BYTES
                )
            } else if result.is_empty() {
                format!("[Command '{}' completed with no output]", command)
            } else {
                result
            }
        }
        Ok(Err(e)) => format!("[Error] Failed to execute '{}': {}", command, e),
        Err(_) => format!("[Error] Command '{}' timed out after 30 seconds]", command),
    }
}

fn execute_search_files(args: &HashMap<String, String>) -> String {
    let pattern = match args.get("pattern") {
        Some(p) => p,
        None => return "[Error] Missing required parameter: pattern".into(),
    };
    let path = args.get("path").map(|s| s.as_str()).unwrap_or(".");

    let full_pattern = if Path::new(pattern).is_absolute() {
        pattern.clone()
    } else {
        format!("{}/{}", path.trim_end_matches('/'), pattern)
    };

    match glob::glob(&full_pattern) {
        Ok(paths) => {
            let results: Vec<String> = paths
                .filter_map(|p| p.ok())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if results.is_empty() {
                "No files matched the pattern.".into()
            } else {
                results.join("\n")
            }
        }
        Err(e) => format!("[Error] Invalid glob pattern '{}': {}", pattern, e),
    }
}

fn execute_search_content(args: &HashMap<String, String>) -> String {
    let pattern = match args.get("pattern") {
        Some(p) => p,
        None => return "[Error] Missing required parameter: pattern".into(),
    };
    let path = args.get("path").map(|s| s.as_str()).unwrap_or(".");
    let include = args.get("include").map(|s| s.as_str());

    let mut cmd_args = vec!["--no-heading", "-n", pattern, path];
    if let Some(include_pattern) = include {
        cmd_args.push("--glob");
        cmd_args.push(include_pattern);
    }

    match Command::new("rg").args(&cmd_args).output() {
        Ok(output) => {
            if output.status.success() {
                let results = String::from_utf8_lossy(&output.stdout);
                if results.len() > 50_000 {
                    let truncated: String = results.chars().take(50_000).collect();
                    format!("{}\n\n[Results truncated at 50,000 chars]", truncated)
                } else {
                    results.to_string()
                }
            } else {
                "No matches found.".into()
            }
        }
        Err(_) => execute_search_content_fallback(pattern, path, include),
    }
}

fn execute_search_content_fallback(pattern: &str, path: &str, include: Option<&str>) -> String {
    let re = match regex::Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => return format!("[Error] Invalid regex pattern '{}': {}", pattern, e),
    };

    let include_re = include.and_then(|i| regex::Regex::new(i).ok());

    let mut results = Vec::new();
    search_dir_fallback(Path::new(path), &re, include_re.as_ref(), &mut results, 0);

    if results.is_empty() {
        "No matches found.".into()
    } else if results.len() > 500 {
        let total = results.len();
        results.truncate(500);
        let mut out = results.join("\n");
        out.push_str(&format!("\n\n[Showing 500 of {} matches]", total));
        out
    } else {
        results.join("\n")
    }
}

fn search_dir_fallback(
    dir: &Path,
    re: &regex::Regex,
    include: Option<&regex::Regex>,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 20 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                if name == "node_modules" || name == ".git" || name == "target" {
                    continue;
                }
                search_dir_fallback(&path, re, include, results, depth + 1);
            } else if let Some(name) = path.to_str() {
                if let Some(inc) = include
                    && !inc.is_match(name)
                {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (line_no, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            results.push(format!("{}:{}: {}", name, line_no + 1, line));
                            if results.len() >= 1000 {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Format a tool result as a message for the conversation.
pub fn format_tool_result_message(result: &ToolResult) -> String {
    format!(
        "[Tool: {}]\nArguments: {}\nOutput:\n{}",
        result.name,
        serde_json::to_string(&result.arguments).unwrap_or_default(),
        result.output
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tool_call() {
        let input = r#"I'll read the file for you.

<read_file path="/tmp/test.txt" />

Let me know what you think."#;

        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments.get("path").unwrap(), "/tmp/test.txt");
    }

    #[test]
    fn parse_multiple_tool_calls() {
        let input = r#"<list_files path="/tmp" />
<read_file path="/tmp/hello.txt" />"#;

        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "list_files");
        assert_eq!(calls[1].name, "read_file");
    }

    #[test]
    fn parse_tool_call_with_multiple_args() {
        let input = r#"<search_content pattern="fn main" path="./src" include="*.rs" />"#;

        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search_content");
        assert_eq!(calls[0].arguments.get("pattern").unwrap(), "fn main");
        assert_eq!(calls[0].arguments.get("path").unwrap(), "./src");
        assert_eq!(calls[0].arguments.get("include").unwrap(), "*.rs");
    }

    #[test]
    fn parse_tool_call_with_text_surrounding() {
        let input = r#"Let me write that file for you.

<write_file path="/tmp/test.txt" content="Hello, World!" />

Done! I've written the file."#;

        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "write_file");
        assert_eq!(calls[0].arguments.get("content").unwrap(), "Hello, World!");
    }

    #[test]
    fn parse_no_tool_calls() {
        let input = "This is just a regular response with no tool calls.";
        let calls = parse_tool_calls(input);
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_ignores_common_tags() {
        let input =
            r#"<response><answer>Use <read_file path="/tmp" /> to read it.</answer></response>"#;
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
    }

    #[test]
    fn is_common_tag_list() {
        assert!(is_common_tag("tool"));
        assert!(is_common_tag("name"));
        assert!(is_common_tag("response"));
        assert!(!is_common_tag("read_file"));
        assert!(!is_common_tag("list_files"));
    }

    #[test]
    fn format_tool_result_message_format() {
        let result = ToolResult {
            name: "read_file".into(),
            arguments: HashMap::from([("path".into(), "/tmp/test.txt".into())]),
            output: "Hello".into(),
            is_error: false,
        };
        let msg = format_tool_result_message(&result);
        assert!(msg.contains("read_file"));
        assert!(msg.contains("path"));
        assert!(msg.contains("Hello"));
    }
}
