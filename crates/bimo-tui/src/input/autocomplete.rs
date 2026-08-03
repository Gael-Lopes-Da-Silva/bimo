use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub type AutocompleteSource = Box<dyn Fn(&str) -> Vec<String> + Send + Sync>;

pub struct Autocomplete {
    sources: Vec<(char, AutocompleteSource)>,
}

impl Autocomplete {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn add_source<F>(mut self, trigger: char, source: F) -> Self
    where
        F: Fn(&str) -> Vec<String> + Send + Sync + 'static,
    {
        self.sources.push((trigger, Box::new(source)));
        self
    }

    pub fn get_suggestions(&self, trigger: char, query: &str) -> Vec<String> {
        for (t, source) in &self.sources {
            if *t == trigger {
                return source(query);
            }
        }
        Vec::new()
    }
}

impl Default for Autocomplete {
    fn default() -> Self {
        Self::new()
            .add_source('/', commands_source())
            .add_source('@', file_path_source())
            .add_source('/', file_path_source())
    }
}

fn commands_source() -> AutocompleteSource {
    Box::new(|query| {
        let commands = vec![
            "exit", "help", "new", "clear", "compact", "fork", "undo", "redo",
        ];
        commands
            .into_iter()
            .filter(|c| c.starts_with(query.trim_start_matches('/')))
            .map(|c| format!("/{}", c))
            .collect()
    })
}

pub struct FileCompleter {
    base_path: PathBuf,
}

impl FileCompleter {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    pub fn complete(&self, query: &str) -> Vec<String> {
        let query = query.trim_start_matches('@').trim_start_matches("./");
        let query_path = self.base_path.join(query);

        let parent = query_path.parent().unwrap_or(&self.base_path);
        let prefix = query_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let mut results = Vec::new();

        if parent.exists() && parent.is_dir() {
            for entry in WalkDir::new(parent)
                .max_depth(1)
                .min_depth(1)
                .into_iter()
                .flatten()
            {
                let name = entry.file_name().to_string_lossy();
                if name.starts_with(prefix) {
                    let mut path = entry
                        .path()
                        .strip_prefix(&self.base_path)
                        .unwrap_or(entry.path())
                        .to_path_buf();
                    if entry.file_type().is_dir() {
                        path.push("");
                    }
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }

        results.sort();
        results
    }
}

fn file_path_source() -> AutocompleteSource {
    let completer = FileCompleter::new(".");
    Box::new(move |query| completer.complete(query))
}

pub fn create_file_completer(base_path: impl AsRef<Path>) -> AutocompleteSource {
    let completer = FileCompleter::new(base_path);
    Box::new(move |query| completer.complete(query))
}
