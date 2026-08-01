use std::fmt::Display;

/// A single command available from the palette.
pub struct PaletteItem {
    pub label: &'static str,
    pub description: &'static str,
}

impl Display for PaletteItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — {}", self.label, self.description)
    }
}

pub const PALETTE_ITEMS: &[PaletteItem] = &[
    PaletteItem {
        label: "Quit",
        description: "Exit bimo",
    },
    PaletteItem {
        label: "Clear output",
        description: "Remove all output lines",
    },
    PaletteItem {
        label: "Close palette",
        description: "Dismiss this palette",
    },
];

/// Returns the items whose label contains `query`, case-insensitively.
/// An empty query returns all items.
pub fn filter<'a>(items: &'a [PaletteItem], query: &str) -> Vec<&'a PaletteItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return items.iter().collect();
    }
    items
        .iter()
        .filter(|item| item.label.to_lowercase().contains(&query))
        .collect()
}
