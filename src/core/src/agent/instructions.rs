use std::path::PathBuf;
use tracing::info;

/// Loads instruction content from standard project files.
/// Order of precedence (first found wins):
///   1. `.agents/` directory in project root (reads all .md files)
///   2. `.agent/` directory in home folder (reads all .md files)
///   3. `AGENTS.md` file in project root
pub fn load_instructions(project_dir: Option<&str>) -> String {
    let mut instructions = String::new();

    // 1. Project-level .agents/ directory
    if let Some(project) = project_dir {
        let agents_dir = PathBuf::from(project).join(".agents");
        if agents_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&agents_dir)
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

    // 2. Home-level .agent/ directory
    if let Some(home) = dirs::home_dir() {
        let agent_dir = home.join(".agent");
        if agent_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&agent_dir)
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

    // 3. Project-level AGENTS.md file
    if let Some(project) = project_dir {
        let agents_md = PathBuf::from(project).join("AGENTS.md");
        if agents_md.is_file()
            && let Ok(content) = std::fs::read_to_string(&agents_md)
        {
            info!("Loaded instructions from AGENTS.md");
            instructions.push_str(&content);
            instructions.push('\n');
        }
    }

    instructions
}
