mod template;

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;

pub use template::render_template;

/// Loads and renders system prompt templates from the prompts/ directory.
/// Templates use `{{PLACEHOLDER}}` notation for variable substitution.
pub struct PromptEngine {
    templates_dir: PathBuf,
    cache: HashMap<String, String>,
}

impl PromptEngine {
    pub fn new(templates_dir: PathBuf) -> Self {
        let mut engine = Self {
            templates_dir,
            cache: HashMap::new(),
        };
        engine.load_all();
        engine
    }

    fn load_all(&mut self) {
        if !self.templates_dir.is_dir() {
            warn!("Prompts directory not found: {:?}", self.templates_dir);
            return;
        }

        let entries = match std::fs::read_dir(&self.templates_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md" || e == "txt" || e == "prompt") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.cache.insert(name, content);
                }
            }
        }
    }

    /// Get a raw template by name (without extension).
    pub fn get_template(&self, name: &str) -> Option<&str> {
        self.cache.get(name).map(|s| s.as_str())
    }

    /// Render a named template with variables.
    pub fn render(&self, name: &str, vars: &HashMap<String, String>) -> Option<String> {
        let template = self.cache.get(name)?;
        Some(render_template(template, vars))
    }

    /// Render the default system prompt with variables.
    pub fn render_default(&self, vars: &HashMap<String, String>) -> String {
        let os = std::env::consts::OS;
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());

        let mut default_vars = HashMap::new();
        default_vars.insert("OS".to_string(), os.to_string());
        default_vars.insert("SHELL".to_string(), shell);
        default_vars.insert(
            "CWD".to_string(),
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        );
        default_vars.insert(
            "PROJECT_LANGUAGE".to_string(),
            Self::detect_project_language(),
        );

        for (k, v) in vars {
            default_vars.insert(k.clone(), v.clone());
        }

        self.render("system_default", &default_vars)
            .unwrap_or_else(|| "You are Bimo, an AI coding agent.".to_string())
    }

    fn detect_project_language() -> String {
        let cwd = std::env::current_dir().ok();
        let dir = cwd.as_deref().unwrap_or(std::path::Path::new("."));

        if dir.join("Cargo.toml").exists() {
            "Rust".to_string()
        } else if dir.join("package.json").exists() {
            "JavaScript / TypeScript".to_string()
        } else if dir.join("pyproject.toml").exists() || dir.join("setup.py").exists() {
            "Python".to_string()
        } else if dir.join("go.mod").exists() {
            "Go".to_string()
        } else if dir.join("Gemfile").exists() {
            "Ruby".to_string()
        } else if dir.join("Makefile").exists() || dir.join("CMakeLists.txt").exists() {
            "C / C++".to_string()
        } else {
            "Unknown".to_string()
        }
    }
}
