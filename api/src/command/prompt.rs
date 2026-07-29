use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct PromptCommand;

impl SlashCommand for PromptCommand {
    fn name(&self) -> &str {
        "prompt"
    }

    fn description(&self) -> &str {
        "load and send a prompt template from .agents/prompts/"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: self.name().to_string(),
            description: self.description().to_string(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all available prompt templates".into(),
                    usage: "/prompt list".into(),
                },
                SubcommandInfo {
                    name: "<name>".into(),
                    description: "load and send the named prompt template".into(),
                    usage: "/prompt <name>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() || args == "help" {
            let templates = list_templates();
            let template_list: String = if templates.is_empty() {
                "\n  (no templates found in .agents/prompts/ or ~/.agents/prompts/)".into()
            } else {
                templates
                    .iter()
                    .map(|t| format!("\n  {:<20} {}", t.name, t.description))
                    .collect()
            };

            return Ok(CommandResult {
                command: "prompt".into(),
                output: format!(
                    "Usage: /prompt <name>\n       /prompt list\n       /prompt help\n\nAvailable templates:{template_list}"
                ),
                data: Some(serde_json::json!({
                    "templates": templates,
                })),
            });
        }

        if args == "list" {
            let templates = list_templates();
            if templates.is_empty() {
                return Ok(CommandResult {
                    command: "prompt".into(),
                    output: "No prompt templates found.".into(),
                    data: Some(serde_json::json!({ "templates": [] })),
                });
            }

            let mut output = String::from("Available prompt templates:\n");
            for t in &templates {
                output.push_str(&format!("  {:<20} {}\n", t.name, t.description));
            }
            output.push_str(&format!(
                "\nUse /prompt <name> to load one. {} template(s) total.",
                templates.len()
            ));

            return Ok(CommandResult {
                command: "prompt".into(),
                output,
                data: Some(serde_json::json!({ "templates": templates })),
            });
        }

        // Load a specific template
        let template_name = args.to_string();
        match load_template(&template_name) {
            Some(template) => {
                let rendered = template.content.trim().to_string();
                ctx.pending_user_message = Some(rendered.clone());

                Ok(CommandResult {
                    command: "prompt".into(),
                    output: format!(
                        "Loaded prompt '{}': {}\n\n{}",
                        template.name, template.description, rendered
                    ),
                    data: Some(serde_json::json!({
                        "template": template,
                        "content": rendered,
                    })),
                })
            }
            None => Err(BimoError::Command(format!(
                "prompt template '{}' not found.\n\
                 Looked in:\n  .agents/prompts/{}.md\n  ~/.agents/prompts/{}.md\n\n\
                 Use /prompt list to see available templates.",
                template_name, template_name, template_name
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Template data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Template discovery and loading
// ---------------------------------------------------------------------------

/// Directories to search for prompt templates, in priority order.
fn template_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Project-level: walk up from CWD looking for .agents/prompts/
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let project = ancestor.join(".agents").join("prompts");
            if project.is_dir() {
                dirs.push(project);
                break;
            }
        }
    }

    // User-level: ~/.agents/prompts/
    if let Some(home) = dirs::home_dir() {
        let user = home.join(".agents").join("prompts");
        if user.is_dir() {
            dirs.push(user);
        }
    }

    dirs
}

/// List all available prompt templates with their descriptions.
pub fn list_templates() -> Vec<PromptTemplate> {
    let mut templates: Vec<PromptTemplate> = Vec::new();

    for dir in template_dirs() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md")
                    && let Some(name) = path.file_stem().and_then(|s| s.to_str())
                    && let Some(t) = parse_template_file(name, &path)
                {
                    // Avoid duplicates: first-found wins (project overrides user)
                    if !templates.iter().any(|t| t.name == name) {
                        templates.push(t);
                    }
                }
            }
        }
    }

    templates
}

/// Load a single template by name (without .md extension).
pub fn load_template(name: &str) -> Option<PromptTemplate> {
    for dir in template_dirs() {
        let path = dir.join(format!("{name}.md"));
        if path.is_file() {
            return parse_template_file(name, &path);
        }
    }
    None
}

/// Parse a .md file with optional YAML frontmatter.
fn parse_template_file(name: &str, path: &Path) -> Option<PromptTemplate> {
    let content = fs::read_to_string(path).ok()?;

    let (description, body) = parse_frontmatter(&content);

    Some(PromptTemplate {
        name: name.to_string(),
        description: description.unwrap_or_else(|| "no description".into()),
        content: body,
    })
}

/// Parse YAML-like frontmatter delimited by `---`.
///
/// Returns (description, body) where description is the value of the
/// `description` field in the frontmatter (if any), and body is everything
/// after the closing `---`.
fn parse_frontmatter(content: &str) -> (Option<String>, String) {
    let content = content.trim();

    if !content.starts_with("---") {
        return (None, content.to_string());
    }

    let after_first = &content[3..].trim_start();

    if let Some(end) = after_first.find("\n---") {
        let frontmatter = after_first[..end].trim();
        let body = after_first[end + 4..].trim().to_string();

        let description = frontmatter.lines().find_map(|line| {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("description:") {
                Some(value.trim().trim_matches('"').trim().to_string())
            } else {
                line.strip_prefix("description :")
                    .map(|value| value.trim().trim_matches('"').trim().to_string())
            }
        });

        (description, body)
    } else {
        // Opening --- but no closing ---, treat as no frontmatter
        (None, content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frontmatter_with_description() {
        let content = "---\ndescription: Review staged git changes\n---\n\nReview the changes.";
        let (desc, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Review staged git changes"));
        assert_eq!(body, "Review the changes.");
    }

    #[test]
    fn parse_frontmatter_no_description() {
        let content = "---\nkey: value\n---\nBody content";
        let (desc, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert_eq!(body, "Body content");
    }

    #[test]
    fn parse_frontmatter_no_frontmatter() {
        let content = "Just a plain markdown file.";
        let (desc, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert_eq!(body, "Just a plain markdown file.");
    }

    #[test]
    fn parse_frontmatter_unclosed() {
        let content = "---\ndescription: test\nNo closing frontmatter";
        let (desc, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn parse_frontmatter_multiline_body() {
        let content = "---\ndescription: Multi-line template\n---\n\nLine 1\nLine 2\nLine 3";
        let (desc, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Multi-line template"));
        assert!(body.contains("Line 1"));
        assert!(body.contains("Line 2"));
        assert!(body.contains("Line 3"));
    }

    #[test]
    fn parse_frontmatter_description_with_quotes() {
        let content = "---\ndescription: \"Quoted description\"\n---\nBody";
        let (desc, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Quoted description"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn load_template_not_found() {
        let template = load_template("nonexistent_template_xyz");
        assert!(template.is_none());
    }
}
