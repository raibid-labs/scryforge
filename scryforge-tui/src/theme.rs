//! Theme definitions for Scryforge TUI.

use fusabi_tui_core::style::Color;

/// A theme defines all the colors used in the application.
#[derive(Debug, Clone)]
pub struct Theme {
    pub foreground: Color,
    pub background: Color,
    pub border: Color,
    pub border_focused: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub accent: Color,
    pub muted: Color,
    pub unread: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dracula()
    }
}

impl Theme {
    /// Dracula theme (default).
    pub fn dracula() -> Self {
        Self {
            foreground: Color::Rgb(248, 248, 242),
            background: Color::Rgb(40, 42, 54),
            border: Color::Rgb(98, 114, 164),
            border_focused: Color::Rgb(189, 147, 249),
            selection_bg: Color::Rgb(68, 71, 90),
            selection_fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(139, 233, 253),
            muted: Color::Rgb(98, 114, 164),
            unread: Color::Rgb(255, 121, 198),
            success: Color::Rgb(80, 250, 123),
            warning: Color::Rgb(241, 250, 140),
            error: Color::Rgb(255, 85, 85),
        }
    }

    /// Nord theme.
    pub fn nord() -> Self {
        Self {
            foreground: Color::Rgb(216, 222, 233),
            background: Color::Rgb(46, 52, 64),
            border: Color::Rgb(76, 86, 106),
            border_focused: Color::Rgb(136, 192, 208),
            selection_bg: Color::Rgb(59, 66, 82),
            selection_fg: Color::Rgb(236, 239, 244),
            accent: Color::Rgb(136, 192, 208),
            muted: Color::Rgb(129, 161, 193),
            unread: Color::Rgb(191, 97, 106),
            success: Color::Rgb(163, 190, 140),
            warning: Color::Rgb(235, 203, 139),
            error: Color::Rgb(191, 97, 106),
        }
    }

    /// Gruvbox theme.
    pub fn gruvbox() -> Self {
        Self {
            foreground: Color::Rgb(235, 219, 178),
            background: Color::Rgb(40, 40, 40),
            border: Color::Rgb(124, 111, 100),
            border_focused: Color::Rgb(254, 128, 25),
            selection_bg: Color::Rgb(60, 56, 54),
            selection_fg: Color::Rgb(251, 241, 199),
            accent: Color::Rgb(131, 165, 152),
            muted: Color::Rgb(146, 131, 116),
            unread: Color::Rgb(211, 134, 155),
            success: Color::Rgb(184, 187, 38),
            warning: Color::Rgb(250, 189, 47),
            error: Color::Rgb(251, 73, 52),
        }
    }

    /// Solarized Dark theme.
    pub fn solarized_dark() -> Self {
        Self {
            foreground: Color::Rgb(131, 148, 150),
            background: Color::Rgb(0, 43, 54),
            border: Color::Rgb(7, 54, 66),
            border_focused: Color::Rgb(38, 139, 210),
            selection_bg: Color::Rgb(7, 54, 66),
            selection_fg: Color::Rgb(147, 161, 161),
            accent: Color::Rgb(42, 161, 152),
            muted: Color::Rgb(88, 110, 117),
            unread: Color::Rgb(211, 54, 130),
            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(181, 137, 0),
            error: Color::Rgb(220, 50, 47),
        }
    }

    /// Tokyo Night theme.
    pub fn tokyo_night() -> Self {
        Self {
            foreground: Color::Rgb(169, 177, 214),
            background: Color::Rgb(26, 27, 38),
            border: Color::Rgb(52, 59, 88),
            border_focused: Color::Rgb(125, 207, 255),
            selection_bg: Color::Rgb(52, 59, 88),
            selection_fg: Color::Rgb(192, 202, 245),
            accent: Color::Rgb(125, 207, 255),
            muted: Color::Rgb(86, 95, 137),
            unread: Color::Rgb(255, 117, 127),
            success: Color::Rgb(158, 206, 106),
            warning: Color::Rgb(224, 175, 104),
            error: Color::Rgb(247, 118, 142),
        }
    }

    /// Get a theme by name.
    pub fn by_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "dracula" => Some(Self::dracula()),
            "nord" => Some(Self::nord()),
            "gruvbox" => Some(Self::gruvbox()),
            "solarized" | "solarized-dark" => Some(Self::solarized_dark()),
            "tokyo-night" | "tokyonight" => Some(Self::tokyo_night()),
            _ => None,
        }
    }

    /// Get list of available theme names.
    pub fn available_themes() -> Vec<String> {
        vec![
            "dracula".to_string(),
            "nord".to_string(),
            "gruvbox".to_string(),
            "solarized-dark".to_string(),
            "tokyo-night".to_string(),
        ]
    }
}
