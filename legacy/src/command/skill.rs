use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};
use crate::skill;

pub(super) struct SkillCommand;

impl SlashCommand for SkillCommand {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "load and manage agent skills from .agents/skills/"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: self.name().to_string(),
            description: self.description().to_string(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all available skills".into(),
                    usage: "/skill list".into(),
                },
                SubcommandInfo {
                    name: "<name>".into(),
                    description: "load the named skill's instructions".into(),
                    usage: "/skill <name>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() || args == "help" {
            let skills = skill::list_skills();
            let skill_list: String = if skills.is_empty() {
                "\n  (no skills found in .agents/skills/ or ~/.agents/skills/)".into()
            } else {
                skills
                    .iter()
                    .map(|s| format!("\n  {:<20} {}", s.name, s.description))
                    .collect()
            };

            return Ok(CommandResult {
                command: "skill".into(),
                output: format!(
                    "Usage: /skill <name>\n       /skill list\n       /skill help\n\nAvailable skills:{skill_list}"
                ),
                data: Some(serde_json::json!({
                    "skills": skills,
                })),
            });
        }

        if args == "list" {
            let skills = skill::list_skills();
            if skills.is_empty() {
                return Ok(CommandResult {
                    command: "skill".into(),
                    output: "No skills found.".into(),
                    data: Some(serde_json::json!({ "skills": [] })),
                });
            }

            let mut output = String::from("Available skills:\n");
            for s in &skills {
                output.push_str(&format!("  {:<20} {}\n", s.name, s.description));
            }
            output.push_str(&format!(
                "\nUse /skill <name> to load one. {} skill(s) total.",
                skills.len()
            ));

            return Ok(CommandResult {
                command: "skill".into(),
                output,
                data: Some(serde_json::json!({ "skills": skills })),
            });
        }

        let skill_name = args.to_string();
        match skill::load_skill(&skill_name) {
            Some(skill) => {
                let content = format!(
                    "[Loaded Skill: {}]\n{}\n\n{}",
                    skill.name, skill.description, skill.content
                );
                ctx.pending_system_message = Some(content.clone());
                ctx.loaded_skills.push(skill.name.clone());

                Ok(CommandResult {
                    command: "skill".into(),
                    output: format!(
                        "Loaded skill '{}': {}\n\n{}",
                        skill.name, skill.description, skill.content
                    ),
                    data: Some(serde_json::json!({
                        "skill": {
                            "name": skill.name,
                            "description": skill.description,
                            "content": skill.content,
                        },
                    })),
                })
            }
            None => Err(BimoError::Command(format!(
                "skill '{}' not found.\n\
                 Looked in:\n  .agents/skills/{}/SKILL.md\n  ~/.agents/skills/{}/SKILL.md\n\n\
                 Use /skill list to see available skills.",
                skill_name, skill_name, skill_name
            ))),
        }
    }
}
