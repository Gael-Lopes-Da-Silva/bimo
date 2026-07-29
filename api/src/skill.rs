use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
}

fn skill_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let project = ancestor.join(".agents").join("skills");
            if project.is_dir() {
                dirs.push(project);
                break;
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let user = home.join(".agents").join("skills");
        if user.is_dir() {
            dirs.push(user);
        }
    }

    dirs
}

pub fn list_skills() -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();

    for dir in skill_dirs() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let skill_file = path.join("SKILL.md");
                if !skill_file.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if skills.iter().any(|s| s.name == name) {
                    continue;
                }
                if let Some(skill) = load_skill_from_dir(&name, &path) {
                    skills.push(skill);
                }
            }
        }
    }

    skills
}

pub fn load_skill(name: &str) -> Option<Skill> {
    for dir in skill_dirs() {
        let skill_dir = dir.join(name);
        if !skill_dir.is_dir() {
            continue;
        }
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        return load_skill_from_dir(name, &skill_dir);
    }
    None
}

fn load_skill_from_dir(name: &str, dir: &Path) -> Option<Skill> {
    let skill_file = dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_file).ok()?;

    let (manifest, body) = parse_skill_manifest(&content);

    Some(Skill {
        name: manifest
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| name.to_string()),
        description: manifest
            .as_ref()
            .map(|m| m.description.clone())
            .unwrap_or_else(|| "no description".into()),
        content: body,
        dir: dir.to_path_buf(),
    })
}

fn parse_skill_manifest(content: &str) -> (Option<SkillManifest>, String) {
    let content = content.trim();

    if !content.starts_with("---") {
        return (None, content.to_string());
    }

    let after_first = &content[3..].trim_start();

    if let Some(end) = after_first.find("\n---") {
        let frontmatter = after_first[..end].trim();
        let body = after_first[end + 4..].trim().to_string();

        let mut name = None;
        let mut description = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(value.trim().trim_matches('"').trim().to_string());
            } else if let Some(value) = line.strip_prefix("name :") {
                name = Some(value.trim().trim_matches('"').trim().to_string());
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(value.trim().trim_matches('"').trim().to_string());
            } else if let Some(value) = line.strip_prefix("description :") {
                description = Some(value.trim().trim_matches('"').trim().to_string());
            }
        }

        let manifest = name.map(|n| SkillManifest {
            name: n,
            description: description.unwrap_or_else(|| "no description".into()),
        });

        (manifest, body)
    } else {
        (None, content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_skill(dir: &Path, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}");
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn parse_skill_manifest_with_all_fields() {
        let content = "---\nname: my-skill\ndescription: Does something\n---\n\nInstructions here.";
        let (manifest, body) = parse_skill_manifest(content);
        assert!(manifest.is_some());
        let m = manifest.unwrap();
        assert_eq!(m.name, "my-skill");
        assert_eq!(m.description, "Does something");
        assert_eq!(body, "Instructions here.");
    }

    #[test]
    fn parse_skill_manifest_name_only() {
        let content = "---\nname: my-skill\n---\n\nBody";
        let (manifest, body) = parse_skill_manifest(content);
        assert!(manifest.is_some());
        let m = manifest.unwrap();
        assert_eq!(m.name, "my-skill");
        assert_eq!(m.description, "no description");
        assert_eq!(body, "Body");
    }

    #[test]
    fn parse_skill_manifest_no_frontmatter() {
        let content = "Just instructions.";
        let (manifest, body) = parse_skill_manifest(content);
        assert!(manifest.is_none());
        assert_eq!(body, "Just instructions.");
    }

    #[test]
    fn parse_skill_manifest_unclosed() {
        let content = "---\nname: my-skill\nNo closing";
        let (manifest, body) = parse_skill_manifest(content);
        assert!(manifest.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn load_skill_from_dir_with_frontmatter() {
        let dir = std::env::temp_dir().join("bimo_skill_test_load");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        create_test_skill(
            &dir,
            "test-skill",
            "A test skill",
            "Run these steps:\n1. Do thing\n2. Profit",
        );

        let skill = load_skill_from_dir("test-skill", &dir.join("test-skill"));
        assert!(skill.is_some());
        let s = skill.unwrap();
        assert_eq!(s.name, "test-skill");
        assert_eq!(s.description, "A test skill");
        assert!(s.content.contains("Do thing"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_skill_from_dir_without_frontmatter() {
        let dir = std::env::temp_dir().join("bimo_skill_test_nofm");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let skill_dir = dir.join("bare-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "Just some instructions").unwrap();

        let skill = load_skill_from_dir("bare-skill", &skill_dir);
        assert!(skill.is_some());
        let s = skill.unwrap();
        assert_eq!(s.name, "bare-skill");
        assert_eq!(s.description, "no description");
        assert_eq!(s.content, "Just some instructions");

        let _ = fs::remove_dir_all(&dir);
    }
}
