use std::path::Path;
use tracing::info;

/// Loads instruction content from project files in the following order:
///   1. Project root files: AGENTS.md, CLAUDE.md, CODEX.md, GEMINI.md, CONTRIBUTING.md
///   2. .github/copilot-instructions.md
///   3. .agents/instructions.md
///   4. .ai/ subdirectories: rules/, context/, workflows/
pub fn load_instructions(project_dir: Option<&str>) -> String {
    let Some(project) = project_dir else {
        return String::new();
    };

    let root = Path::new(project);
    let mut instructions = String::new();

    // 1. Project root files
    for filename in &[
        "AGENTS.md",
        "CLAUDE.md",
        "CODEX.md",
        "GEMINI.md",
        "CONTRIBUTING.md",
    ] {
        let path = root.join(filename);
        if path.is_file()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            info!("Loaded instructions from {filename}");
            instructions.push_str(&content);
            instructions.push('\n');
        }
    }

    // 2. .github/copilot-instructions.md
    let copilot = root.join(".github").join("copilot-instructions.md");
    if copilot.is_file()
        && let Ok(content) = std::fs::read_to_string(&copilot)
    {
        info!("Loaded instructions from .github/copilot-instructions.md");
        instructions.push_str(&content);
        instructions.push('\n');
    }

    // 3. .agents/instructions.md
    let agents_instructions = root.join(".agents").join("instructions.md");
    if agents_instructions.is_file()
        && let Ok(content) = std::fs::read_to_string(&agents_instructions)
    {
        info!("Loaded instructions from .agents/instructions.md");
        instructions.push_str(&content);
        instructions.push('\n');
    }

    // 4. .ai/ subdirectories (rules/, context/, workflows/)
    for subdir in &["rules", "context", "workflows"] {
        let dir = root.join(".ai").join(subdir);
        if dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md")
                    && let Ok(content) = std::fs::read_to_string(&path)
                {
                    info!("Loaded instructions from {:?}", path);
                    instructions.push_str(&content);
                    instructions.push('\n');
                }
            }
        }
    }

    instructions
}
