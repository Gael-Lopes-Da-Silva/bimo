use cursive::theme::{BaseColor, BorderStyle, Color, PaletteColor, Theme};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeVariant {
    Dark,
    Light,
}

impl ThemeVariant {
    pub fn detect() -> Self {
        if is_light_terminal() {
            ThemeVariant::Light
        } else {
            ThemeVariant::Dark
        }
    }
}

fn is_light_terminal() -> bool {
    if let Ok(colorterm) = std::env::var("COLORTERM")
        && (colorterm.contains("truecolor") || colorterm.contains("24bit"))
    {
        return false;
    }
    if let Ok(term) = std::env::var("TERM")
        && term.contains("light")
    {
        return true;
    }
    if let Ok(theme) = std::env::var("TERM_THEME")
        && theme.to_lowercase().contains("light")
    {
        return true;
    }
    false
}

#[derive(Debug, Clone)]
pub struct BimoTheme {
    pub variant: ThemeVariant,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub primary: Color,
    pub success: Color,
    pub error: Color,
    pub muted: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub border: Color,
    pub input_bg: Color,
    pub input_fg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::dark()
    }
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            background: Color::TerminalDefault,
            surface: Color::Rgb(30, 30, 30),
            surface_alt: Color::Rgb(40, 40, 40),
            primary: Color::Rgb(100, 180, 255),
            success: Color::Rgb(80, 200, 120),
            error: Color::Rgb(255, 85, 85),
            muted: Color::Rgb(120, 120, 120),
            text: Color::Rgb(230, 230, 230),
            text_secondary: Color::Rgb(170, 170, 170),
            border: Color::Rgb(60, 60, 60),
            input_bg: Color::Rgb(25, 25, 25),
            input_fg: Color::Rgb(230, 230, 230),
            selection_bg: Color::Rgb(60, 100, 160),
            selection_fg: Color::Rgb(255, 255, 255),
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::TerminalDefault,
            surface: Color::Rgb(245, 245, 245),
            surface_alt: Color::Rgb(235, 235, 235),
            primary: Color::Rgb(0, 100, 200),
            success: Color::Rgb(0, 160, 80),
            error: Color::Rgb(220, 50, 50),
            muted: Color::Rgb(140, 140, 140),
            text: Color::Rgb(30, 30, 30),
            text_secondary: Color::Rgb(90, 90, 90),
            border: Color::Rgb(200, 200, 200),
            input_bg: Color::Rgb(255, 255, 255),
            input_fg: Color::Rgb(30, 30, 30),
            selection_bg: Color::Rgb(200, 220, 255),
            selection_fg: Color::Rgb(0, 0, 0),
        }
    }

    pub fn with_overrides(mut self, overrides: HashMap<String, Color>) -> Self {
        for (key, color) in overrides {
            match key.as_str() {
                "background" => self.background = color,
                "surface" => self.surface = color,
                "surface_alt" => self.surface_alt = color,
                "primary" => self.primary = color,
                "success" => self.success = color,
                "error" => self.error = color,
                "muted" => self.muted = color,
                "text" => self.text = color,
                "text_secondary" => self.text_secondary = color,
                "border" => self.border = color,
                "input_bg" => self.input_bg = color,
                "input_fg" => self.input_fg = color,
                "selection_bg" => self.selection_bg = color,
                "selection_fg" => self.selection_fg = color,
                _ => {}
            }
        }
        self
    }
}

impl BimoTheme {
    pub fn new(variant: ThemeVariant) -> Self {
        let colors = match variant {
            ThemeVariant::Dark => ThemeColors::dark(),
            ThemeVariant::Light => ThemeColors::light(),
        };
        Self { variant, colors }
    }

    pub fn with_config(variant: ThemeVariant, config: HashMap<String, Color>) -> Self {
        let colors = match variant {
            ThemeVariant::Dark => ThemeColors::dark().with_overrides(config),
            ThemeVariant::Light => ThemeColors::light().with_overrides(config),
        };
        Self { variant, colors }
    }

    pub fn to_cursive_theme(&self) -> Theme {
        let mut theme = Theme {
            shadow: false,
            borders: BorderStyle::None,
            ..Theme::default()
        };

        let palette = &mut theme.palette;
        palette[PaletteColor::Background] = self.colors.background;
        palette[PaletteColor::View] = self.colors.surface;
        palette[PaletteColor::Primary] = self.colors.primary;
        palette[PaletteColor::Secondary] = self.colors.surface_alt;
        palette[PaletteColor::Tertiary] = self.colors.muted;
        palette[PaletteColor::Highlight] = self.colors.selection_bg;
        palette[PaletteColor::HighlightText] = self.colors.selection_fg;

        theme
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("Config directory not found")]
    ConfigDirNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn parse_color(s: &str) -> Result<Color, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex")?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex")?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex")?;
            return Ok(Color::Rgb(r, g, b));
        }
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).map_err(|_| "Invalid hex")?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).map_err(|_| "Invalid hex")?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).map_err(|_| "Invalid hex")?;
            return Ok(Color::Rgb(r, g, b));
        }
    }
    if s.parse::<u8>().is_ok() {
        return Ok(Color::TerminalDefault);
    }
    match s.to_lowercase().as_str() {
        "default" => Ok(Color::TerminalDefault),
        "black" => Ok(Color::Dark(BaseColor::Black)),
        "red" => Ok(Color::Dark(BaseColor::Red)),
        "green" => Ok(Color::Dark(BaseColor::Green)),
        "yellow" => Ok(Color::Dark(BaseColor::Yellow)),
        "blue" => Ok(Color::Dark(BaseColor::Blue)),
        "magenta" => Ok(Color::Dark(BaseColor::Magenta)),
        "cyan" => Ok(Color::Dark(BaseColor::Cyan)),
        "white" => Ok(Color::Dark(BaseColor::White)),
        "bright_black" => Ok(Color::Light(BaseColor::Black)),
        "bright_red" => Ok(Color::Light(BaseColor::Red)),
        "bright_green" => Ok(Color::Light(BaseColor::Green)),
        "bright_yellow" => Ok(Color::Light(BaseColor::Yellow)),
        "bright_blue" => Ok(Color::Light(BaseColor::Blue)),
        "bright_magenta" => Ok(Color::Light(BaseColor::Magenta)),
        "bright_cyan" => Ok(Color::Light(BaseColor::Cyan)),
        "bright_white" => Ok(Color::Light(BaseColor::White)),
        _ => Err(format!("Unknown color: {}", s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_color("#ff0000"), Ok(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("#0f0"), Ok(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_color("#0000ff"), Ok(Color::Rgb(0, 0, 255)));
        assert_eq!(
            parse_color("not-a-color"),
            Err("Unknown color: not-a-color".into())
        );
    }

    #[test]
    fn test_parse_named_color() {
        assert_eq!(parse_color("red"), Ok(Color::Dark(BaseColor::Red)));
        assert_eq!(
            parse_color("BRIGHT_BLUE"),
            Ok(Color::Light(BaseColor::Blue))
        );
        assert_eq!(parse_color("default"), Ok(Color::TerminalDefault));
    }

    #[test]
    fn test_theme_variant_is_valid() {
        match ThemeVariant::detect() {
            ThemeVariant::Dark | ThemeVariant::Light => {}
        }
    }
}
