use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub rendered: String,
    pub git_branch: Option<String>,
    pub agent_instruction_files: Vec<String>,
}

impl ProjectContext {
    pub fn is_empty(&self) -> bool {
        self.rendered == "No project context available."
    }
}

pub fn build_project_context(cwd: &str) -> ProjectContext {
    let mut parts: Vec<String> = Vec::new();
    let mut git_branch: Option<String> = None;

    if let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
    {
        if out.status.success() {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !branch.is_empty() {
                git_branch = Some(branch.clone());
                parts.push(format!("Git branch: {branch}"));
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(cwd) {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();
        names.sort();
        if !names.is_empty() {
            parts.push(format!("Project files: {}", names.join(", ")));
        }
    }

    let (instruction_files, instruction_chunks) = load_agent_instructions(cwd);
    for chunk in instruction_chunks {
        parts.push(chunk);
    }

    let rendered = if parts.is_empty() {
        "No project context available.".into()
    } else {
        parts.join("\n")
    };

    ProjectContext {
        rendered,
        git_branch,
        agent_instruction_files: instruction_files,
    }
}

pub fn load_agent_instructions(cwd: &str) -> (Vec<String>, Vec<String>) {
    const CANDIDATES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".agents/AGENTS.md"];

    let mut filenames: Vec<String> = Vec::new();
    let mut chunks: Vec<String> = Vec::new();

    for &file in CANDIDATES {
        let path = Path::new(cwd).join(file);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    filenames.push(file.to_string());
                    chunks.push(format!("Instructions from {file}:\n{trimmed}"));
                }
            }
        }
    }

    (filenames, chunks)
}
