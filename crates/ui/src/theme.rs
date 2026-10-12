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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

/// Geometry and typography scale parameters (§2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryTokens {
    pub tab_height: f32,
    pub tree_row_height: f32,
    pub status_height: f32,
    pub sidebar_default_width: f32,
    pub markdown_max_width: f32,
    pub corner_radius: f32,
    pub border_width: f32,
    pub ui_font_size: f32,
    pub editor_font_size: f32,
}

impl GeometryTokens {
    pub const fn compact() -> Self {
        Self {
            tab_height: 32.0,
            tree_row_height: 28.0,
            status_height: 26.0,
            sidebar_default_width: 260.0,
            markdown_max_width: 800.0,
            corner_radius: 5.0,
            border_width: 1.0,
            ui_font_size: 13.0,
            editor_font_size: 14.0,
        }
    }

    pub const fn comfortable() -> Self {
        Self {
            tab_height: 36.0,
            tree_row_height: 32.0,
            status_height: 28.0,
            sidebar_default_width: 280.0,
            markdown_max_width: 850.0,
            corner_radius: 6.0,
            border_width: 1.0,
            ui_font_size: 14.0,
            editor_font_size: 15.0,
        }
    }
}

/// Visual theme design tokens following Obsidian's sleek palette and design system (§8.2, §2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    // Semantic tokens (§2.2)
    /// Main editor canvas background (background.canvas).
    pub bg_canvas: Color,
    /// Sidebar / navigation panel background (background.panel).
    pub bg_panel: Color,
    /// Floating / modal / elevated component background (background.elevated).
    pub bg_elevated: Color,
    /// Selected item background (background.selected).
    pub bg_selected: Color,
    /// Subtle border / divider color (border.subtle).
    pub border_subtle: Color,

    /// Primary text color (text.primary).
    pub text_primary: Color,
    /// Secondary text color for descriptions and subtitles (text.secondary).
    pub text_secondary: Color,
    /// Muted text for line numbers and paths (text.muted).
    pub text_muted: Color,
    /// Accent color for active elements (accent.primary).
    pub accent_primary: Color,

    // Status colors
    pub state_error: Color,
    pub state_warning: Color,
    pub state_success: Color,

    // Compatibility aliases with existing theme fields
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_tertiary: Color,
    pub bg_surface: Color,
    pub border: Color,
    pub accent: Color,
    pub selection: Color,
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

    pub density: Density,
    pub geometry: GeometryTokens,
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
    pub density: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::nord()
    }
}

impl Theme {
    /// Standard Nord theme (default) inspired by Obsidian and Zed.
    pub fn nord() -> Self {
        let bg_canvas = Color::from_hex("#2d333f").unwrap();
        let bg_panel = Color::from_hex("#252a34").unwrap();
        let bg_tertiary = Color::from_hex("#1b1e24").unwrap();
        let bg_elevated = Color::from_hex("#252a34").unwrap();
        let border_subtle = Color::from_hex("#333945").unwrap();
        let text_primary = Color::from_hex("#e2e6ed").unwrap();
        let text_secondary = Color::from_hex("#a8b2c1").unwrap();
        let text_muted = Color::from_hex("#8b95a5").unwrap();
        let accent = Color::from_hex("#88a9c1").unwrap();
        let selection = Color::from_hex("#394351").unwrap();

        Self {
            bg_canvas,
            bg_panel,
            bg_elevated,
            bg_selected: selection,
            border_subtle,

            text_primary,
            text_secondary,
            text_muted,
            accent_primary: accent,

            state_error: Color::from_hex("#bf616a").unwrap(),
            state_warning: Color::from_hex("#ebcb8b").unwrap(),
            state_success: Color::from_hex("#a3be8c").unwrap(),

            bg_primary: bg_canvas,
            bg_secondary: bg_panel,
            bg_tertiary,
            bg_surface: bg_elevated,
            border: border_subtle,
            accent,
            selection,
            cursor: accent,

            callout_note: Color::from_hex("#81a1c1").unwrap(),
            callout_warning: Color::from_hex("#ebcb8b").unwrap(),
            callout_tip: Color::from_hex("#a3be8c").unwrap(),
            callout_important: Color::from_hex("#bf616a").unwrap(),

            flash_bg: Color::from_hex("#ebcb8b").unwrap(),
            flash_fg: Color::from_hex("#20242c").unwrap(),

            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,

            density: Density::Compact,
            geometry: GeometryTokens::compact(),
        }
    }

    /// Standard Obsidian-inspired dark theme.
    pub fn obsidian_dark() -> Self {
        let bg_canvas = Color::from_hex("#1e1e1e").unwrap();
        let bg_panel = Color::from_hex("#181818").unwrap();
        let bg_tertiary = Color::from_hex("#141414").unwrap();
        let bg_elevated = Color::from_hex("#242424").unwrap();
        let border_subtle = Color::from_hex("#2b2b2b").unwrap();
        let text_primary = Color::from_hex("#dcddde").unwrap();
        let text_secondary = Color::from_hex("#a6a7ab").unwrap();
        let text_muted = Color::from_hex("#8a8b91").unwrap();
        let accent = Color::from_hex("#7c3aed").unwrap();
        let selection = Color::from_hex("#333333").unwrap();

        Self {
            bg_canvas,
            bg_panel,
            bg_elevated,
            bg_selected: selection,
            border_subtle,

            text_primary,
            text_secondary,
            text_muted,
            accent_primary: accent,

            state_error: Color::from_hex("#e05252").unwrap(),
            state_warning: Color::from_hex("#e5a83b").unwrap(),
            state_success: Color::from_hex("#3ba55d").unwrap(),

            bg_primary: bg_canvas,
            bg_secondary: bg_panel,
            bg_tertiary,
            bg_surface: bg_elevated,
            border: border_subtle,
            accent,
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

            density: Density::Compact,
            geometry: GeometryTokens::compact(),
        }
    }

    /// Applies custom theme overrides on top of this theme.
    pub fn apply_custom(&self, custom: &CustomThemeConfig) -> Self {
        let mut theme = self.clone();
        if let Some(ref c) = custom.bg_primary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_primary = parsed;
            theme.bg_canvas = parsed;
        }
        if let Some(ref c) = custom.bg_secondary
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.bg_secondary = parsed;
            theme.bg_panel = parsed;
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
            theme.bg_elevated = parsed;
        }
        if let Some(ref c) = custom.border
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.border = parsed;
            theme.border_subtle = parsed;
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
            theme.accent_primary = parsed;
        }
        if let Some(ref c) = custom.selection
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.selection = parsed;
            theme.bg_selected = parsed;
        }
        if let Some(ref c) = custom.cursor
            && let Some(parsed) = Color::from_hex(c)
        {
            theme.cursor = parsed;
        }
        if let Some(ref d) = custom.density {
            if d == "comfortable" {
                theme.density = Density::Comfortable;
                theme.geometry = GeometryTokens::comfortable();
            } else if d == "compact" {
                theme.density = Density::Compact;
                theme.geometry = GeometryTokens::compact();
            }
        }
        theme
    }

    /// Obsidian-inspired clean light theme.
    pub fn light() -> Self {
        let bg_canvas = Color::rgb(0.98, 0.98, 0.99);
        let bg_panel = Color::rgb(0.94, 0.94, 0.96);
        let bg_tertiary = Color::rgb(0.90, 0.90, 0.92);
        let bg_elevated = Color::rgb(1.0, 1.0, 1.0);
        let border_subtle = Color::rgb(0.82, 0.83, 0.85);
        let text_primary = Color::rgb(0.15, 0.16, 0.18);
        let text_secondary = Color::rgb(0.35, 0.36, 0.40);
        let text_muted = Color::rgb(0.50, 0.52, 0.55);
        let accent = Color::rgb(0.45, 0.20, 0.88);
        let selection = Color::rgba(0.45, 0.20, 0.88, 0.25);

        Self {
            bg_canvas,
            bg_panel,
            bg_elevated,
            bg_selected: selection,
            border_subtle,

            text_primary,
            text_secondary,
            text_muted,
            accent_primary: accent,

            state_error: Color::rgb(0.85, 0.20, 0.25),
            state_warning: Color::rgb(0.85, 0.50, 0.05),
            state_success: Color::rgb(0.10, 0.65, 0.35),

            bg_primary: bg_canvas,
            bg_secondary: bg_panel,
            bg_tertiary,
            bg_surface: bg_elevated,
            border: border_subtle,
            accent,
            selection,
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

            density: Density::Compact,
            geometry: GeometryTokens::compact(),
        }
    }

    /// Returns `true` if current theme is dark.
    pub fn is_dark(&self) -> bool {
        self.bg_primary.r < 0.5
    }
}
