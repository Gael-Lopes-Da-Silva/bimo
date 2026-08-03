use crate::theme::{BimoTheme, ThemeError, ThemeVariant};
use cursive::theme::Color;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ThemeConfigFile {
    pub name: String,
    pub variant: Option<ThemeVariant>,
    pub colors: Option<HashMap<String, String>>,
}

impl ThemeConfigFile {
    pub fn to_bimo_theme(&self) -> BimoTheme {
        let variant = self.variant.unwrap_or_else(ThemeVariant::detect);
        let colors = self
            .colors
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| crate::theme::parse_color(&v).ok().map(|c| (k, c)))
            .collect();
        BimoTheme::with_config(variant, colors)
    }
}

pub fn list_available_themes() -> Result<Vec<String>, ThemeError> {
    let config_dir = get_themes_dir()?;
    let mut themes = Vec::new();

    if config_dir.exists() {
        for entry in std::fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                themes.push(name.to_string());
            }
        }
    }

    themes.sort();
    Ok(themes)
}

pub fn load_theme(name: Option<&str>) -> Result<BimoTheme, ThemeError> {
    let theme_name = name.unwrap_or("default");
    let config_dir = get_themes_dir()?;
    let theme_file = config_dir.join(format!("{}.json", theme_name));

    if !theme_file.exists() {
        return Ok(BimoTheme::new(ThemeVariant::detect()));
    }

    let content = std::fs::read_to_string(&theme_file)?;
    let config: ThemeConfigFile = serde_json::from_str(&content)?;

    Ok(config.to_bimo_theme())
}

pub fn save_theme(name: &str, theme: &BimoTheme) -> Result<(), ThemeError> {
    let config_dir = get_themes_dir()?;
    std::fs::create_dir_all(&config_dir)?;

    let colors_map: HashMap<String, String> = [
        (
            "background".to_string(),
            format_color(theme.colors.background),
        ),
        ("surface".to_string(), format_color(theme.colors.surface)),
        (
            "surface_alt".to_string(),
            format_color(theme.colors.surface_alt),
        ),
        ("primary".to_string(), format_color(theme.colors.primary)),
        ("success".to_string(), format_color(theme.colors.success)),
        ("error".to_string(), format_color(theme.colors.error)),
        ("muted".to_string(), format_color(theme.colors.muted)),
        ("text".to_string(), format_color(theme.colors.text)),
        (
            "text_secondary".to_string(),
            format_color(theme.colors.text_secondary),
        ),
        ("border".to_string(), format_color(theme.colors.border)),
        ("input_bg".to_string(), format_color(theme.colors.input_bg)),
        ("input_fg".to_string(), format_color(theme.colors.input_fg)),
        (
            "selection_bg".to_string(),
            format_color(theme.colors.selection_bg),
        ),
        (
            "selection_fg".to_string(),
            format_color(theme.colors.selection_fg),
        ),
    ]
    .into_iter()
    .collect();

    let config = ThemeConfigFile {
        name: name.to_string(),
        variant: Some(theme.variant),
        colors: Some(colors_map),
    };

    let content = serde_json::to_string_pretty(&config)?;
    let theme_file = config_dir.join(format!("{}.json", name));
    std::fs::write(theme_file, content)?;

    Ok(())
}

fn format_color(c: Color) -> String {
    match c {
        Color::TerminalDefault => "default".to_string(),
        Color::Dark(b) => format!("{:?}", b).to_lowercase(),
        Color::Light(b) => format!("bright_{:?}", b).to_lowercase(),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::RgbLowRes(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
    }
}

pub fn get_themes_dir() -> Result<PathBuf, ThemeError> {
    dirs::config_dir()
        .ok_or(ThemeError::ConfigDirNotFound)
        .map(|p| p.join("bimo").join("themes"))
}

pub fn ensure_default_theme() -> Result<(), ThemeError> {
    let config_dir = get_themes_dir()?;
    std::fs::create_dir_all(&config_dir)?;

    let default_file = config_dir.join("default.json");
    if !default_file.exists() {
        let default_theme = BimoTheme::new(ThemeVariant::detect());
        save_theme("default", &default_theme)?;
    }

    Ok(())
}
