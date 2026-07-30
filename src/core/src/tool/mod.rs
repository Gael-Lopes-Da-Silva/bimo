pub mod execute;

use serde::{Deserialize, Serialize};
use tool_parser::parsers::json::JsonParser;
use tool_parser::traits::ToolParser;
use tool_parser::types::ToolCall as ParserToolCall;

use crate::error::{BimoError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

impl Tool {
    pub fn to_xml(&self) -> String {
        let mut xml = format!(
            "<tool name=\"{name}\" description=\"{desc}\">\n",
            name = self.name,
            desc = self.description
        );
        xml.push_str("  <parameters>\n");
        for p in &self.parameters {
            xml.push_str(&format!(
                "    <parameter name=\"{name}\" type=\"{ptype}\" required=\"{req}\">{desc}</parameter>\n",
                name = p.name,
                ptype = p.param_type,
                req = if p.required { "true" } else { "false" },
                desc = p.description,
            ));
        }
        xml.push_str("  </parameters>\n");
        xml.push_str("</tool>");
        xml
    }

    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for p in &self.parameters {
            let schema = serde_json::json!({
                "type": p.param_type,
                "description": p.description,
            });
            properties.insert(p.name.clone(), schema);
            if p.required {
                required.push(p.name.clone());
            }
        }
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required,
                },
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ParsedToolCall {
    pub fn get_arg(&self, name: &str) -> Option<&serde_json::Value> {
        self.arguments.get(name)
    }

    pub fn get_arg_string(&self, name: &str) -> Option<String> {
        self.arguments
            .get(name)
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    pub fn get_arg_number(&self, name: &str) -> Option<f64> {
        self.arguments.get(name).and_then(|v| v.as_f64())
    }
}

pub struct ToolRegistry {
    tools: Vec<Tool>,
    parser: JsonParser,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            parser: JsonParser::new(),
        }
    }

    pub fn register(&mut self, tool: Tool) {
        self.tools.push(tool);
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn render_xml(&self) -> String {
        self.tools
            .iter()
            .map(|t| t.to_xml())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn render_json_schemas(&self) -> Vec<serde_json::Value> {
        self.tools.iter().map(|t| t.to_json_schema()).collect()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    pub fn parse_tool_calls(&mut self, text: &str) -> Result<Vec<ParsedToolCall>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BimoError::Api(format!("Failed to build runtime: {e}")))?;

        rt.block_on(async {
            let (_, calls) = self
                .parser
                .parse_complete(text)
                .await
                .map_err(|e| BimoError::Api(format!("Tool parse error: {e}")))?;
            Ok(calls
                .into_iter()
                .map(|tc: ParserToolCall| {
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                    ParsedToolCall {
                        name: tc.function.name,
                        arguments: args,
                    }
                })
                .collect())
        })
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_tools() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    registry.register(Tool {
        name: "read_file".into(),
        description: "Read the contents of a file at the given path.".into(),
        parameters: vec![ToolParameter {
            name: "path".into(),
            description: "Absolute path to the file to read.".into(),
            param_type: "string".into(),
            required: true,
        }],
    });

    registry.register(Tool {
        name: "edit_file".into(),
        description: "Edit a file by replacing an existing string with a new one.".into(),
        parameters: vec![
            ToolParameter {
                name: "path".into(),
                description: "Absolute path to the file to edit.".into(),
                param_type: "string".into(),
                required: true,
            },
            ToolParameter {
                name: "old_string".into(),
                description: "The exact existing text to replace.".into(),
                param_type: "string".into(),
                required: true,
            },
            ToolParameter {
                name: "new_string".into(),
                description: "The new text to insert in place of old_string.".into(),
                param_type: "string".into(),
                required: true,
            },
        ],
    });

    registry.register(Tool {
        name: "write_file".into(),
        description: "Write full content to a file, overwriting if it exists.".into(),
        parameters: vec![
            ToolParameter {
                name: "path".into(),
                description: "Absolute path to the file to write.".into(),
                param_type: "string".into(),
                required: true,
            },
            ToolParameter {
                name: "content".into(),
                description: "Full content to write to the file.".into(),
                param_type: "string".into(),
                required: true,
            },
        ],
    });

    registry.register(Tool {
        name: "run_command".into(),
        description: "Execute a shell command and return its output.".into(),
        parameters: vec![
            ToolParameter {
                name: "command".into(),
                description: "Shell command to execute.".into(),
                param_type: "string".into(),
                required: true,
            },
            ToolParameter {
                name: "timeout".into(),
                description: "Maximum execution time in seconds (optional).".into(),
                param_type: "number".into(),
                required: false,
            },
        ],
    });

    registry.register(Tool {
        name: "manage_todo".into(),
        description: "Manage the todo list. Actions: add, update, remove.".into(),
        parameters: vec![
            ToolParameter {
                name: "action".into(),
                description: "Action to perform: add, update, remove.".into(),
                param_type: "string".into(),
                required: true,
            },
            ToolParameter {
                name: "id".into(),
                description: "Todo item ID (for update/remove).".into(),
                param_type: "string".into(),
                required: false,
            },
            ToolParameter {
                name: "description".into(),
                description: "Todo item description (for add/update).".into(),
                param_type: "string".into(),
                required: false,
            },
            ToolParameter {
                name: "status".into(),
                description: "Todo item status: pending, in_progress, done.".into(),
                param_type: "string".into(),
                required: false,
            },
        ],
    });

    registry
}
