use ratatui::prelude::{Color, Style, Stylize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeVariant {
    Mocha,
    Latte,
    Frappe,
    Macchiato,
}

impl Default for ThemeVariant {
    fn default() -> Self {
        Self::Mocha
    }
}

impl ThemeVariant {
    pub const ALL: &[ThemeVariant] = &[
        ThemeVariant::Mocha,
        ThemeVariant::Latte,
        ThemeVariant::Frappe,
        ThemeVariant::Macchiato,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ThemeVariant::Mocha => "Mocha",
            ThemeVariant::Latte => "Latte",
            ThemeVariant::Frappe => "Frappe",
            ThemeVariant::Macchiato => "Macchiato",
        }
    }

    pub fn is_dark(&self) -> bool {
        matches!(
            self,
            ThemeVariant::Mocha | ThemeVariant::Frappe | ThemeVariant::Macchiato
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,
    pub overlay0: Color,
    pub overlay1: Color,
    pub overlay2: Color,
    pub subtext0: Color,
    pub subtext1: Color,
    pub text: Color,
    pub lavender: Color,
    pub blue: Color,
    pub sapphire: Color,
    pub sky: Color,
    pub teal: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub maroon: Color,
    pub red: Color,
    pub mauve: Color,
    pub pink: Color,
    pub flamingo: Color,
    pub rosewater: Color,
}

impl Theme {
    pub fn mocha() -> Self {
        Self {
            base: Color::Rgb(0x1e, 0x1e, 0x2e),
            mantle: Color::Rgb(0x18, 0x18, 0x25),
            crust: Color::Rgb(0x11, 0x11, 0x1b),
            surface0: Color::Rgb(0x31, 0x32, 0x44),
            surface1: Color::Rgb(0x45, 0x47, 0x5a),
            surface2: Color::Rgb(0x58, 0x5b, 0x70),
            overlay0: Color::Rgb(0x6c, 0x70, 0x86),
            overlay1: Color::Rgb(0x7f, 0x84, 0x9c),
            overlay2: Color::Rgb(0x93, 0x99, 0xb2),
            subtext0: Color::Rgb(0xa6, 0xad, 0xc8),
            subtext1: Color::Rgb(0xba, 0xc1, 0xdc),
            text: Color::Rgb(0xcd, 0xd6, 0xf4),
            lavender: Color::Rgb(0xb4, 0xbe, 0xfe),
            blue: Color::Rgb(0x89, 0xb4, 0xfa),
            sapphire: Color::Rgb(0x74, 0xc7, 0xec),
            sky: Color::Rgb(0x89, 0xd2, 0xea),
            teal: Color::Rgb(0x94, 0xe2, 0xd5),
            green: Color::Rgb(0xa6, 0xe3, 0xa1),
            yellow: Color::Rgb(0xf9, 0xe2, 0xaf),
            peach: Color::Rgb(0xfa, 0xb2, 0x74),
            maroon: Color::Rgb(0xeb, 0xa8, 0x8f),
            red: Color::Rgb(0xf3, 0x8b, 0xa8),
            mauve: Color::Rgb(0xc9, 0xa9, 0xe2),
            pink: Color::Rgb(0xf5, 0xc2, 0xe7),
            flamingo: Color::Rgb(0xf2, 0xcd, 0xcd),
            rosewater: Color::Rgb(0xf5, 0xe0, 0xdc),
        }
    }

    pub fn latte() -> Self {
        Self {
            base: Color::Rgb(0xef, 0xf1, 0xf5),
            mantle: Color::Rgb(0xe6, 0xe9, 0xef),
            crust: Color::Rgb(0xdc, 0xdf, 0xe8),
            surface0: Color::Rgb(0xcc, 0xd0, 0xda),
            surface1: Color::Rgb(0xbc, 0xc0, 0xcc),
            surface2: Color::Rgb(0xac, 0xb0, 0xbe),
            overlay0: Color::Rgb(0x9c, 0xa0, 0xae),
            overlay1: Color::Rgb(0x8c, 0x8f, 0xa0),
            overlay2: Color::Rgb(0x7c, 0x7f, 0x90),
            subtext0: Color::Rgb(0x6c, 0x6f, 0x80),
            subtext1: Color::Rgb(0x5c, 0x5f, 0x70),
            text: Color::Rgb(0x4c, 0x4f, 0x60),
            lavender: Color::Rgb(0x72, 0x87, 0xfd),
            blue: Color::Rgb(0x1e, 0x66, 0xf5),
            sapphire: Color::Rgb(0x20, 0x9f, 0xb5),
            sky: Color::Rgb(0x04, 0xa5, 0xe5),
            teal: Color::Rgb(0x17, 0x92, 0x99),
            green: Color::Rgb(0x40, 0xa0, 0x2b),
            yellow: Color::Rgb(0xdf, 0x8e, 0x1d),
            peach: Color::Rgb(0xfe, 0x64, 0x0b),
            maroon: Color::Rgb(0xe6, 0x45, 0x53),
            red: Color::Rgb(0xd2, 0x0f, 0x39),
            mauve: Color::Rgb(0x88, 0x39, 0xef),
            pink: Color::Rgb(0xea, 0x76, 0xcb),
            flamingo: Color::Rgb(0xdd, 0x78, 0x78),
            rosewater: Color::Rgb(0xc1, 0x4a, 0x4a),
        }
    }

    pub fn frappe() -> Self {
        Self {
            base: Color::Rgb(0x30, 0x34, 0x46),
            mantle: Color::Rgb(0x29, 0x2c, 0x3c),
            crust: Color::Rgb(0x23, 0x26, 0x34),
            surface0: Color::Rgb(0x41, 0x45, 0x59),
            surface1: Color::Rgb(0x51, 0x57, 0x6d),
            surface2: Color::Rgb(0x62, 0x68, 0x80),
            overlay0: Color::Rgb(0x73, 0x79, 0x94),
            overlay1: Color::Rgb(0x83, 0x8b, 0xa7),
            overlay2: Color::Rgb(0x94, 0x9c, 0xb8),
            subtext0: Color::Rgb(0xa5, 0xad, 0xcb),
            subtext1: Color::Rgb(0xb5, 0xbd, 0xdb),
            text: Color::Rgb(0xc6, 0xd0, 0xf5),
            lavender: Color::Rgb(0xca, 0x9e, 0xed),
            blue: Color::Rgb(0x8c, 0xaa, 0xee),
            sapphire: Color::Rgb(0x85, 0xc3, 0xd9),
            sky: Color::Rgb(0x99, 0xd1, 0xdb),
            teal: Color::Rgb(0x81, 0xc8, 0xbe),
            green: Color::Rgb(0xa6, 0xd1, 0x89),
            yellow: Color::Rgb(0xe5, 0xc8, 0x90),
            peach: Color::Rgb(0xef, 0x9f, 0x76),
            maroon: Color::Rgb(0xea, 0x99, 0x9c),
            red: Color::Rgb(0xe7, 0x82, 0x84),
            mauve: Color::Rgb(0xca, 0x9e, 0xed),
            pink: Color::Rgb(0xf4, 0xb8, 0xe4),
            flamingo: Color::Rgb(0xec, 0xc2, 0xc2),
            rosewater: Color::Rgb(0xf2, 0xd5, 0xcf),
        }
    }

    pub fn macchiato() -> Self {
        Self {
            base: Color::Rgb(0x24, 0x27, 0x3a),
            mantle: Color::Rgb(0x1e, 0x20, 0x30),
            crust: Color::Rgb(0x18, 0x19, 0x26),
            surface0: Color::Rgb(0x36, 0x3a, 0x51),
            surface1: Color::Rgb(0x47, 0x4d, 0x68),
            surface2: Color::Rgb(0x5b, 0x60, 0x7e),
            overlay0: Color::Rgb(0x6e, 0x73, 0x93),
            overlay1: Color::Rgb(0x80, 0x87, 0xa6),
            overlay2: Color::Rgb(0x93, 0x9b, 0xb8),
            subtext0: Color::Rgb(0xa5, 0xad, 0xce),
            subtext1: Color::Rgb(0xb8, 0xc0, 0xe0),
            text: Color::Rgb(0xca, 0xd3, 0xf5),
            lavender: Color::Rgb(0xb7, 0xbd, 0xf8),
            blue: Color::Rgb(0x8a, 0xad, 0xf4),
            sapphire: Color::Rgb(0x7d, 0xc4, 0xe4),
            sky: Color::Rgb(0x91, 0xd7, 0xe3),
            teal: Color::Rgb(0x8b, 0xd5, 0xc0),
            green: Color::Rgb(0xa6, 0xda, 0x95),
            yellow: Color::Rgb(0xee, 0xd4, 0x9f),
            peach: Color::Rgb(0xf5, 0xa9, 0x7f),
            maroon: Color::Rgb(0xee, 0x99, 0xa0),
            red: Color::Rgb(0xed, 0x87, 0x96),
            mauve: Color::Rgb(0xc6, 0xa0, 0xf6),
            pink: Color::Rgb(0xf5, 0xb8, 0xec),
            flamingo: Color::Rgb(0xf0, 0xc6, 0xc6),
            rosewater: Color::Rgb(0xf4, 0xdb, 0xd6),
        }
    }

    pub fn from_variant(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Mocha => Self::mocha(),
            ThemeVariant::Latte => Self::latte(),
            ThemeVariant::Frappe => Self::frappe(),
            ThemeVariant::Macchiato => Self::macchiato(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Styles {
    pub base: Style,
    pub title: Style,
    pub border: Style,
    pub border_focus: Style,
    pub text: Style,
    pub text_muted: Style,
    pub text_dim: Style,
    pub selected: Style,
    pub selected_text: Style,
    pub button: Style,
    pub button_hover: Style,
    pub button_active: Style,
    pub button_disabled: Style,
    pub input: Style,
    pub input_focus: Style,
    pub scrollbar: Style,
    pub scrollbar_thumb: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub info: Style,
    pub user_msg: Style,
    pub assistant_msg: Style,
    pub tool_msg: Style,
    pub system_msg: Style,
    pub code_bg: Style,
    pub link: Style,
    pub keybind: Style,
    pub primary: Style,
}

impl Styles {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            base: Style::default().fg(theme.text).bg(theme.base),
            title: Style::default().fg(theme.text).bg(theme.base).bold(),
            border: Style::default().fg(theme.surface1).bg(theme.base),
            border_focus: Style::default().fg(theme.blue).bg(theme.base),
            text: Style::default().fg(theme.text).bg(theme.base),
            text_muted: Style::default().fg(theme.overlay1).bg(theme.base),
            text_dim: Style::default().fg(theme.overlay0).bg(theme.base),
            selected: Style::default().fg(theme.base).bg(theme.blue),
            selected_text: Style::default().fg(theme.base).bg(theme.blue).bold(),
            button: Style::default().fg(theme.text).bg(theme.surface0),
            button_hover: Style::default().fg(theme.base).bg(theme.blue),
            button_active: Style::default().fg(theme.base).bg(theme.sapphire),
            button_disabled: Style::default().fg(theme.overlay0).bg(theme.surface0),
            input: Style::default().fg(theme.text).bg(theme.mantle),
            input_focus: Style::default().fg(theme.text).bg(theme.mantle),
            scrollbar: Style::default().fg(theme.surface0).bg(theme.base),
            scrollbar_thumb: Style::default().fg(theme.surface2).bg(theme.base),
            success: Style::default().fg(theme.green).bg(theme.base),
            warning: Style::default().fg(theme.yellow).bg(theme.base),
            error: Style::default().fg(theme.red).bg(theme.base),
            info: Style::default().fg(theme.blue).bg(theme.base),
            user_msg: Style::default().fg(theme.text).bg(theme.mantle),
            assistant_msg: Style::default().fg(theme.text).bg(theme.surface0),
            tool_msg: Style::default().fg(theme.text).bg(theme.surface1),
            system_msg: Style::default().fg(theme.overlay1).bg(theme.crust),
            code_bg: Style::default().fg(theme.text).bg(theme.crust),
            link: Style::default()
                .fg(theme.sapphire)
                .bg(theme.base)
                .underlined(),
            keybind: Style::default().fg(theme.mauve).bg(theme.base).bold(),
            primary: Style::default().fg(theme.blue).bg(theme.base),
        }
    }
}
