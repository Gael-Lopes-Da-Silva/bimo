use crate::palette::command::Command;
use cursive::event::Event;
use std::collections::HashMap;

pub struct CommandRegistry {
    commands: HashMap<String, Command>,
    order: Vec<String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register(&mut self, command: Command) {
        let id = command.id.clone();
        if !self.commands.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.commands.insert(id, command);
    }

    pub fn get(&self, id: &str) -> Option<&Command> {
        self.commands.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Command> {
        self.order.retain(|i| i != id);
        self.commands.remove(id)
    }

    pub fn all(&self) -> Vec<&Command> {
        self.order
            .iter()
            .filter_map(|id| self.commands.get(id))
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&Command> {
        if query.is_empty() {
            return self.all();
        }
        self.all()
            .into_iter()
            .filter(|c| c.matches(query))
            .collect()
    }

    pub fn execute(&self, id: &str, siv: &mut cursive::Cursive) -> bool {
        if let Some(cmd) = self.commands.get(id) {
            cmd.execute(siv);
            true
        } else {
            false
        }
    }

    pub fn shortcut_for(&self, event: &Event) -> Option<&Command> {
        self.all()
            .into_iter()
            .find(|c| c.shortcut.as_ref() == Some(event))
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_default_commands();
        registry
    }
}

impl Clone for CommandRegistry {
    fn clone(&self) -> Self {
        let mut new = Self::new();
        for cmd in self.all() {
            new.register(cmd.clone());
        }
        new
    }
}

impl CommandRegistry {
    fn register_default_commands(&mut self) {
        self.register(
            Command::new("exit", "Exit", "Close the application", |siv| siv.quit())
                .with_shortcut(Event::CtrlChar('q')),
        );
        self.register(Command::new("help", "Help", "Show help information", |siv| {
            siv.add_layer(cursive::views::Dialog::info("Bimo TUI Help\n\nCtrl+P - Command Palette\nCtrl+J - New line in input\nEnter - Submit\nEsc - Cancel/Close\nCtrl+Q - Quit"));
        }).with_shortcut(Event::Key(cursive::event::Key::F1)));
        self.register(Command::new(
            "clear",
            "Clear Output",
            "Clear the output area",
            |siv| {
                siv.call_on_name("messages", |messages: &mut cursive::views::LinearLayout| {
                    while messages.get_child(0).is_some() {
                        messages.remove_child(0);
                    }
                });
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut registry = CommandRegistry::new();
        registry.register(Command::new("test", "Test", "A test command", |_| {}));
        assert!(registry.get("test").is_some());
        assert_eq!(registry.get("test").map(|c| c.name.as_str()), Some("Test"));
    }

    #[test]
    fn test_search_by_name_and_description() {
        let mut registry = CommandRegistry::new();
        registry.register(Command::new(
            "compile",
            "Compile",
            "Build the project",
            |_| {},
        ));
        registry.register(Command::new(
            "test",
            "Run tests",
            "Execute the test suite",
            |_| {},
        ));

        assert_eq!(registry.search("compile").len(), 1);
        assert_eq!(registry.search("build").len(), 1);
        assert_eq!(registry.search("nope").len(), 0);
        assert_eq!(registry.search("").len(), 2);
    }

    #[test]
    fn test_default_registry_has_builtins() {
        let registry = CommandRegistry::default();
        assert!(registry.get("exit").is_some());
        assert!(registry.get("help").is_some());
        assert!(registry.get("clear").is_some());
    }
}
