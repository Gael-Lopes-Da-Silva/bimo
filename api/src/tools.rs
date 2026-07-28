use serde::{Deserialize, Serialize};

/// A tool available to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

/// A parameter that a tool accepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: String,
}

/// Registry of all available tools.
pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = Self { tools: Vec::new() };
        reg.register_builtin_tools();
        reg
    }

    fn register_builtin_tools(&mut self) {
        self.tools.push(Tool {
            name: "read_file".into(),
            description: "Read the contents of a file at the given path".into(),
            parameters: vec![ToolParameter {
                name: "path".into(),
                description: "Absolute or relative path to the file".into(),
                required: true,
                parameter_type: "string".into(),
            }],
        });

        self.tools.push(Tool {
            name: "write_file".into(),
            description: "Write content to a file, creating it if it doesn't exist".into(),
            parameters: vec![
                ToolParameter {
                    name: "path".into(),
                    description: "Absolute or relative path to the file".into(),
                    required: true,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "content".into(),
                    description: "The content to write".into(),
                    required: true,
                    parameter_type: "string".into(),
                },
            ],
        });

        self.tools.push(Tool {
            name: "list_files".into(),
            description: "List files and directories at the given path".into(),
            parameters: vec![ToolParameter {
                name: "path".into(),
                description: "Directory path to list (defaults to current directory)".into(),
                required: false,
                parameter_type: "string".into(),
            }],
        });

        self.tools.push(Tool {
            name: "run_command".into(),
            description: "Execute a shell command and return its output".into(),
            parameters: vec![ToolParameter {
                name: "command".into(),
                description: "The shell command to execute".into(),
                required: true,
                parameter_type: "string".into(),
            }],
        });

        self.tools.push(Tool {
            name: "search_files".into(),
            description: "Search for files matching a glob pattern".into(),
            parameters: vec![
                ToolParameter {
                    name: "pattern".into(),
                    description: "Glob pattern to match (e.g. \"**/*.rs\")".into(),
                    required: true,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "path".into(),
                    description: "Directory to search in (defaults to current directory)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
            ],
        });

        self.tools.push(Tool {
            name: "search_content".into(),
            description: "Search file contents for a regex pattern".into(),
            parameters: vec![
                ToolParameter {
                    name: "pattern".into(),
                    description: "Regex pattern to search for".into(),
                    required: true,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "path".into(),
                    description: "Directory to search in (defaults to current directory)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "include".into(),
                    description: "File pattern to include (e.g. \"*.rs\")".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
            ],
        });
    }

    pub fn register(&mut self, tool: Tool) {
        self.tools.push(tool);
    }

    pub fn list(&self) -> &[Tool] {
        &self.tools
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_tools_registered() {
        let reg = ToolRegistry::new();
        assert_eq!(reg.tools.len(), 6);
    }

    #[test]
    fn builtin_tool_names() {
        let reg = ToolRegistry::new();
        let names: Vec<&str> = reg.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"run_command"));
        assert!(names.contains(&"search_files"));
        assert!(names.contains(&"search_content"));
    }

    #[test]
    fn get_tool_by_name() {
        let reg = ToolRegistry::new();
        assert!(reg.get("read_file").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn read_file_has_required_path() {
        let reg = ToolRegistry::new();
        let tool = reg.get("read_file").unwrap();
        assert_eq!(tool.parameters.len(), 1);
        assert_eq!(tool.parameters[0].name, "path");
        assert!(tool.parameters[0].required);
    }

    #[test]
    fn write_file_has_two_required_params() {
        let reg = ToolRegistry::new();
        let tool = reg.get("write_file").unwrap();
        assert_eq!(tool.parameters.len(), 2);
        assert!(tool.parameters.iter().all(|p| p.required));
    }

    #[test]
    fn list_files_has_optional_path() {
        let reg = ToolRegistry::new();
        let tool = reg.get("list_files").unwrap();
        assert_eq!(tool.parameters.len(), 1);
        assert!(!tool.parameters[0].required);
    }

    #[test]
    fn register_custom_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Tool {
            name: "custom".into(),
            description: "a custom tool".into(),
            parameters: vec![],
        });
        assert!(reg.get("custom").is_some());
        assert_eq!(reg.tools.len(), 7);
    }

    #[test]
    fn tools_are_serializable() {
        let reg = ToolRegistry::new();
        let json = serde_json::to_string(&reg.tools).unwrap();
        let deserialized: Vec<Tool> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), reg.tools.len());
    }
}
