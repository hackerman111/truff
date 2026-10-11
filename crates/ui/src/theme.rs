/// Color representation for Truff v2 UI elements (sRGBA).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Converts color to 8-bit RGBA channels for Parley and Vello rendering.
    pub fn to_rgba_u8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.b.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }

    /// Parses a 6-digit hex color `#RRGGBB` or 8-digit `#RRGGBBAA`.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let trimmed = hex.trim().trim_start_matches('#');
        if trimmed.len() == 6 {
            let r = u8::from_str_radix(&trimmed[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&trimmed[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&trimmed[4..6], 16).ok()? as f32 / 255.0;
            Some(Self::rgb(r, g, b))
        } else if trimmed.len() == 8 {
            let r = u8::from_str_radix(&trimmed[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&trimmed[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&trimmed[4..6], 16).ok()? as f32 / 255.0;
            let a = u8::from_str_radix(&trimmed[6..8], 16).ok()? as f32 / 255.0;
            Some(Self::rgba(r, g, b, a))
        } else {
            None
        }
    }
}

/// Visual theme design tokens following Obsidian's sleek palette and design system (§8.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Main editor background.
    pub bg_primary: Color,
    /// Sidebar / navigation background.
    pub bg_secondary: Color,
    /// Right panel / statusline background.
    pub bg_tertiary: Color,
    /// Surface / card background for popups and modals.
    pub bg_surface: Color,
    /// Panel border color.
    pub border: Color,
    /// Primary text color.
    pub text_primary: Color,
    /// Muted text for line numbers and paths.
    pub text_muted: Color,
    /// Accent color for active tabs and highlights.
    pub accent: Color,
    /// Selection highlight background.
    pub selection: Color,
    /// Cursor / caret color.
    pub cursor: Color,

    // Callout accent colors
    pub callout_note: Color,
    pub callout_warning: Color,
    pub callout_tip: Color,
    pub callout_important: Color,

    // Flash navigation badge colors
    pub flash_bg: Color,
    pub flash_fg: Color,

    // Spacing scale tokens (4, 8, 12, 16, 24 px)
    pub spacing_xs: f32,
    pub spacing_sm: f32,
    pub spacing_md: f32,
    pub spacing_lg: f32,
    pub spacing_xl: f32,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomThemeConfig {
    pub bg_primary: Option<String>,
    pub bg_secondary: Option<String>,
    pub bg_tertiary: Option<String>,
    pub bg_surface: Option<String>,
    pub border: Option<String>,
    pub text_primary: Option<String>,
    pub text_muted: Option<String>,
    pub accent: Option<String>,
    pub selection: Option<String>,
    pub cursor: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::nord()
    }
}

impl Theme {
    /// Standard Nord theme (default) inspired by Obsidian Nord.
    pub fn nord() -> Self {
        Self {
            bg_primary: Color::from_hex("#2e3440").unwrap(),
            bg_secondary: Color::from_hex("#242933").unwrap(),
            bg_tertiary: Color::from_hex("#1f232a").unwrap(),
            bg_surface: Color::from_hex("#3b4252").unwrap(),
            border: Color::from_hex("#3b4252").unwrap(),
            text_primary: Color::from_hex("#eceff4").unwrap(),
            text_muted: Color::from_hex("#9aa5b5").unwrap(),
            accent: Color::from_hex("#88c0d0").unwrap(),
            selection: Color::from_hex("#434c5e73").unwrap(),
            cursor: Color::from_hex("#88c0d0").unwrap(),

            callout_note: Color::from_hex("#81a1c1").unwrap(),
            callout_warning: Color::from_hex("#ebcb8b").unwrap(),
            callout_tip: Color::from_hex("#a3be8c").unwrap(),
            callout_important: Color::from_hex("#bf616a").unwrap(),

            flash_bg: Color::from_hex("#ebcb8b").unwrap(),
            flash_fg: Color::from_hex("#2e3440").unwrap(),

            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Standard Obsidian-inspired dark theme.
    pub fn obsidian_dark() -> Self {
        Self {
            bg_primary: Color::from_hex("#1e1e1e").unwrap(),
            bg_secondary: Color::from_hex("#181818").unwrap(),
            bg_tertiary: Color::from_hex("#141414").unwrap(),
            bg_surface: Color::from_hex("#262626").unwrap(),
            border: Color::from_hex("#2b2b2b").unwrap(),
            text_primary: Color::from_hex("#dcddde").unwrap(),
            text_muted: Color::from_hex("#8a8b91").unwrap(),
            accent: Color::from_hex("#7c3aed").unwrap(),
            selection: Color::from_hex("#7c3aed47").unwrap(),
            cursor: Color::from_hex("#9353ff").unwrap(),

            callout_note: Color::rgb(0.20, 0.50, 0.95),
            callout_warning: Color::rgb(0.90, 0.60, 0.10),
            callout_tip: Color::rgb(0.10, 0.75, 0.45),
            callout_important: Color::rgb(0.85, 0.20, 0.25),

            flash_bg: Color::rgb(0.95, 0.75, 0.10),
            flash_fg: Color::rgb(0.05, 0.05, 0.05),

            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Applies custom theme overrides on top of this theme.
    pub fn apply_custom(&self, custom: &CustomThemeConfig) -> Self {
        let mut theme = self.clone();
        if let Some(ref c) = custom.bg_primary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_primary = parsed;
        }
        if let Some(ref c) = custom.bg_secondary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_secondary = parsed;
        }
        if let Some(ref c) = custom.bg_tertiary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_tertiary = parsed;
        }
        if let Some(ref c) = custom.bg_surface
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_surface = parsed;
        }
        if let Some(ref c) = custom.border
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.border = parsed;
        }
        if let Some(ref c) = custom.text_primary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.text_primary = parsed;
        }
        if let Some(ref c) = custom.text_muted
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.text_muted = parsed;
        }
        if let Some(ref c) = custom.accent
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.accent = parsed;
        }
        if let Some(ref c) = custom.selection
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.selection = parsed;
        }
        if let Some(ref c) = custom.cursor
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.cursor = parsed;
        }
        theme
    }

    /// Obsidian-inspired clean light theme.
    pub fn light() -> Self {
        Self {
            bg_primary: Color::rgb(0.98, 0.98, 0.99),
            bg_secondary: Color::rgb(0.94, 0.94, 0.96),
            bg_tertiary: Color::rgb(0.90, 0.90, 0.92),
            bg_surface: Color::rgb(1.0, 1.0, 1.0),
            border: Color::rgb(0.82, 0.83, 0.85),
            text_primary: Color::rgb(0.15, 0.16, 0.18),
            text_muted: Color::rgb(0.50, 0.52, 0.55),
            accent: Color::rgb(0.45, 0.20, 0.88),
            selection: Color::rgba(0.45, 0.20, 0.88, 0.25),
            cursor: Color::rgb(0.10, 0.10, 0.12),

            callout_note: Color::rgb(0.15, 0.45, 0.85),
            callout_warning: Color::rgb(0.85, 0.50, 0.05),
            callout_tip: Color::rgb(0.08, 0.65, 0.35),
            callout_important: Color::rgb(0.80, 0.15, 0.20),

            flash_bg: Color::rgb(0.90, 0.65, 0.05),
            flash_fg: Color::rgb(0.02, 0.02, 0.02),

            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Returns `true` if current theme is dark.
    pub fn is_dark(&self) -> bool {
        self.bg_primary.r < 0.5
    }
}
