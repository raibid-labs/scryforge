//! Theme system for TUI widgets.
//!
//! This module provides a flexible theming system with built-in themes
//! and support for custom themes via configuration.

use ratatui::style::Color;
use std::collections::HashMap;

/// Theme configuration for widgets with comprehensive color palette.
#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub unread: Color,
    pub saved: Color,
    pub error: Color,
    pub success: Color,
    pub border: Color,
    pub border_focused: Color,
    pub header: Color,
    pub muted: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme (default) - inspired by modern terminal aesthetics.
    pub fn dark() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Reset,
            accent: Color::Cyan,
            selection_bg: Color::DarkGray,
            selection_fg: Color::White,
            unread: Color::Yellow,
            saved: Color::Cyan,
            error: Color::Red,
            success: Color::Green,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            header: Color::Cyan,
            muted: Color::DarkGray,
            warning: Color::Yellow,
        }
    }

    /// Light theme - for bright terminal backgrounds.
    pub fn light() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Black,
            accent: Color::Blue,
            selection_bg: Color::LightBlue,
            selection_fg: Color::Black,
            unread: Color::Magenta,
            saved: Color::Blue,
            error: Color::Red,
            success: Color::Green,
            border: Color::Gray,
            border_focused: Color::Blue,
            header: Color::Blue,
            muted: Color::Gray,
            warning: Color::Magenta,
        }
    }

    /// Dracula theme - popular dark theme.
    pub fn dracula() -> Self {
        Self {
            background: Color::Rgb(40, 42, 54),
            foreground: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(139, 233, 253), // Cyan
            selection_bg: Color::Rgb(68, 71, 90),
            selection_fg: Color::Rgb(248, 248, 242),
            unread: Color::Rgb(241, 250, 140), // Yellow
            saved: Color::Rgb(255, 121, 198),  // Pink
            error: Color::Rgb(255, 85, 85),    // Red
            success: Color::Rgb(80, 250, 123), // Green
            border: Color::Rgb(98, 114, 164),
            border_focused: Color::Rgb(139, 233, 253),
            header: Color::Rgb(189, 147, 249), // Purple
            muted: Color::Rgb(98, 114, 164),
            warning: Color::Rgb(241, 250, 140),
        }
    }

    /// Gruvbox theme - retro warm color scheme.
    pub fn gruvbox() -> Self {
        Self {
            background: Color::Rgb(40, 40, 40),
            foreground: Color::Rgb(235, 219, 178),
            accent: Color::Rgb(184, 187, 38), // Green
            selection_bg: Color::Rgb(80, 73, 69),
            selection_fg: Color::Rgb(251, 241, 199),
            unread: Color::Rgb(250, 189, 47),  // Yellow
            saved: Color::Rgb(131, 165, 152),  // Aqua
            error: Color::Rgb(251, 73, 52),    // Red
            success: Color::Rgb(184, 187, 38), // Green
            border: Color::Rgb(146, 131, 116),
            border_focused: Color::Rgb(184, 187, 38),
            header: Color::Rgb(211, 134, 155), // Purple
            muted: Color::Rgb(146, 131, 116),
            warning: Color::Rgb(250, 189, 47),
        }
    }

    /// Nord theme - cold, arctic color palette.
    pub fn nord() -> Self {
        Self {
            background: Color::Rgb(46, 52, 64),
            foreground: Color::Rgb(236, 239, 244),
            accent: Color::Rgb(136, 192, 208), // Frost cyan
            selection_bg: Color::Rgb(59, 66, 82),
            selection_fg: Color::Rgb(236, 239, 244),
            unread: Color::Rgb(235, 203, 139),  // Aurora yellow
            saved: Color::Rgb(163, 190, 140),   // Aurora green
            error: Color::Rgb(191, 97, 106),    // Aurora red
            success: Color::Rgb(163, 190, 140), // Aurora green
            border: Color::Rgb(76, 86, 106),
            border_focused: Color::Rgb(136, 192, 208),
            header: Color::Rgb(129, 161, 193), // Frost blue
            muted: Color::Rgb(76, 86, 106),
            warning: Color::Rgb(235, 203, 139),
        }
    }

    /// Solarized Dark theme - carefully designed contrast.
    pub fn solarized_dark() -> Self {
        Self {
            background: Color::Rgb(0, 43, 54),
            foreground: Color::Rgb(131, 148, 150),
            accent: Color::Rgb(42, 161, 152), // Cyan
            selection_bg: Color::Rgb(7, 54, 66),
            selection_fg: Color::Rgb(147, 161, 161),
            unread: Color::Rgb(181, 137, 0),  // Yellow
            saved: Color::Rgb(108, 113, 196), // Violet
            error: Color::Rgb(220, 50, 47),   // Red
            success: Color::Rgb(133, 153, 0), // Green
            border: Color::Rgb(88, 110, 117),
            border_focused: Color::Rgb(42, 161, 152),
            header: Color::Rgb(38, 139, 210), // Blue
            muted: Color::Rgb(88, 110, 117),
            warning: Color::Rgb(181, 137, 0),
        }
    }

    /// Monokai theme - vibrant and popular.
    pub fn monokai() -> Self {
        Self {
            background: Color::Rgb(39, 40, 34),
            foreground: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(102, 217, 239), // Cyan
            selection_bg: Color::Rgb(73, 72, 62),
            selection_fg: Color::Rgb(248, 248, 242),
            unread: Color::Rgb(230, 219, 116), // Yellow
            saved: Color::Rgb(174, 129, 255),  // Purple
            error: Color::Rgb(249, 38, 114),   // Pink/Red
            success: Color::Rgb(166, 226, 46), // Green
            border: Color::Rgb(117, 113, 94),
            border_focused: Color::Rgb(102, 217, 239),
            header: Color::Rgb(174, 129, 255), // Purple
            muted: Color::Rgb(117, 113, 94),
            warning: Color::Rgb(230, 219, 116),
        }
    }

    /// Get a theme by name.
    pub fn by_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "default" | "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            "dracula" => Some(Self::dracula()),
            "gruvbox" => Some(Self::gruvbox()),
            "nord" => Some(Self::nord()),
            "solarized" | "solarized-dark" | "solarized_dark" => Some(Self::solarized_dark()),
            "monokai" => Some(Self::monokai()),
            _ => None,
        }
    }

    /// Get all available theme names.
    pub fn available_themes() -> Vec<&'static str> {
        vec![
            "default",
            "light",
            "dracula",
            "gruvbox",
            "nord",
            "solarized-dark",
            "monokai",
        ]
    }

    /// Load theme from configuration map.
    ///
    /// The config map should contain color values as strings. Colors can be:
    /// - Named colors: "red", "blue", "cyan", etc.
    /// - RGB hex: "#FF5733" or "FF5733"
    /// - RGB values: "rgb(255, 87, 51)"
    pub fn from_config(config: &HashMap<String, String>) -> Option<Self> {
        // If a theme name is provided, use that as base
        if let Some(base_name) = config.get("base") {
            let mut theme = Self::by_name(base_name)?;

            // Override individual colors if provided
            if let Some(bg) = config.get("background").and_then(|c| parse_color(c)) {
                theme.background = bg;
            }
            if let Some(fg) = config.get("foreground").and_then(|c| parse_color(c)) {
                theme.foreground = fg;
            }
            if let Some(accent) = config.get("accent").and_then(|c| parse_color(c)) {
                theme.accent = accent;
            }
            // ... more color overrides can be added

            Some(theme)
        } else {
            // Build theme from scratch if no base provided
            Some(Self {
                background: config
                    .get("background")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Reset),
                foreground: config
                    .get("foreground")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Reset),
                accent: config
                    .get("accent")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Cyan),
                selection_bg: config
                    .get("selection_bg")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::DarkGray),
                selection_fg: config
                    .get("selection_fg")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::White),
                unread: config
                    .get("unread")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Yellow),
                saved: config
                    .get("saved")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Cyan),
                error: config
                    .get("error")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Red),
                success: config
                    .get("success")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Green),
                border: config
                    .get("border")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::DarkGray),
                border_focused: config
                    .get("border_focused")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Cyan),
                header: config
                    .get("header")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Cyan),
                muted: config
                    .get("muted")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::DarkGray),
                warning: config
                    .get("warning")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Color::Yellow),
            })
        }
    }
}

/// Parse a color string into a ratatui Color.
fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().to_lowercase();

    // Named colors
    match s.as_str() {
        "reset" => return Some(Color::Reset),
        "black" => return Some(Color::Black),
        "red" => return Some(Color::Red),
        "green" => return Some(Color::Green),
        "yellow" => return Some(Color::Yellow),
        "blue" => return Some(Color::Blue),
        "magenta" => return Some(Color::Magenta),
        "cyan" => return Some(Color::Cyan),
        "gray" | "grey" => return Some(Color::Gray),
        "darkgray" | "darkgrey" => return Some(Color::DarkGray),
        "lightred" => return Some(Color::LightRed),
        "lightgreen" => return Some(Color::LightGreen),
        "lightyellow" => return Some(Color::LightYellow),
        "lightblue" => return Some(Color::LightBlue),
        "lightmagenta" => return Some(Color::LightMagenta),
        "lightcyan" => return Some(Color::LightCyan),
        "white" => return Some(Color::White),
        _ => {}
    }

    // Hex color: #RRGGBB or RRGGBB
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Some(Color::Rgb(r, g, b));
            }
        }
    } else if s.len() == 6 {
        // Try parsing as hex without #
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Some(Color::Rgb(r, g, b));
        }
    }

    // RGB format: rgb(r, g, b)
    if s.starts_with("rgb(") && s.ends_with(')') {
        let rgb_str = &s[4..s.len() - 1];
        let parts: Vec<&str> = rgb_str.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                return Some(Color::Rgb(r, g, b));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.accent, Color::Cyan);
        assert_eq!(theme.unread, Color::Yellow);
    }

    #[test]
    fn test_theme_by_name() {
        assert!(Theme::by_name("dark").is_some());
        assert!(Theme::by_name("light").is_some());
        assert!(Theme::by_name("dracula").is_some());
        assert!(Theme::by_name("gruvbox").is_some());
        assert!(Theme::by_name("nord").is_some());
        assert!(Theme::by_name("solarized-dark").is_some());
        assert!(Theme::by_name("monokai").is_some());
        assert!(Theme::by_name("nonexistent").is_none());
    }

    #[test]
    fn test_available_themes() {
        let themes = Theme::available_themes();
        assert!(themes.contains(&"default"));
        assert!(themes.contains(&"light"));
        assert!(themes.contains(&"dracula"));
        assert!(themes.contains(&"gruvbox"));
    }

    #[test]
    fn test_parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
        assert_eq!(parse_color("lightblue"), Some(Color::LightBlue));
    }

    #[test]
    fn test_parse_color_hex() {
        assert_eq!(parse_color("#FF5733"), Some(Color::Rgb(255, 87, 51)));
        assert_eq!(parse_color("FF5733"), Some(Color::Rgb(255, 87, 51)));
        assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn test_parse_color_rgb() {
        assert_eq!(
            parse_color("rgb(255, 87, 51)"),
            Some(Color::Rgb(255, 87, 51))
        );
        assert_eq!(parse_color("rgb(0, 0, 0)"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn test_parse_color_invalid() {
        assert_eq!(parse_color("invalid"), None);
        assert_eq!(parse_color("#ZZZ"), None);
        assert_eq!(parse_color("rgb(300, 0, 0)"), None); // Out of range
    }

    #[test]
    fn test_theme_from_config_with_base() {
        let mut config = HashMap::new();
        config.insert("base".to_string(), "dracula".to_string());

        let theme = Theme::from_config(&config);
        assert!(theme.is_some());
    }

    #[test]
    fn test_theme_consistency() {
        // Ensure all themes can be created without panicking
        let themes = [
            "dark",
            "light",
            "dracula",
            "gruvbox",
            "nord",
            "solarized-dark",
            "monokai",
        ];
        for name in &themes {
            assert!(
                Theme::by_name(name).is_some(),
                "Theme {} should exist",
                name
            );
        }
    }

    #[test]
    fn test_dracula_theme_colors() {
        let theme = Theme::dracula();
        // Test a few key colors to ensure theme is properly configured
        assert!(matches!(theme.background, Color::Rgb(40, 42, 54)));
        assert!(matches!(theme.foreground, Color::Rgb(248, 248, 242)));
    }

    #[test]
    fn test_gruvbox_theme_colors() {
        let theme = Theme::gruvbox();
        assert!(matches!(theme.background, Color::Rgb(40, 40, 40)));
    }
}
