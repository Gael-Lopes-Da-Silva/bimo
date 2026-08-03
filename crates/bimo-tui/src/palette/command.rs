use cursive::Cursive;
use cursive::event::Event;
use std::sync::Arc;

pub struct Command {
    pub id: String,
    pub name: String,
    pub description: String,
    pub shortcut: Option<Event>,
    pub action: Arc<dyn Fn(&mut Cursive) + Send + Sync>,
}

impl Command {
    pub fn new<F>(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        action: F,
    ) -> Self
    where
        F: Fn(&mut Cursive) + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            shortcut: None,
            action: Arc::new(action),
        }
    }

    pub fn with_shortcut(mut self, event: Event) -> Self {
        self.shortcut = Some(event);
        self
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.name.to_lowercase().contains(&query)
            || self.description.to_lowercase().contains(&query)
            || self.id.to_lowercase().contains(&query)
    }

    pub fn shortcut_display(&self) -> String {
        self.shortcut
            .as_ref()
            .map(|e| format!("{:?}", e))
            .unwrap_or_default()
    }

    pub fn execute(&self, siv: &mut Cursive) {
        (self.action)(siv);
    }
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("shortcut", &self.shortcut)
            .finish()
    }
}

impl Clone for Command {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            shortcut: self.shortcut.clone(),
            action: self.action.clone(),
        }
    }
}
