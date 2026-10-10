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

impl Default for Theme {
    fn default() -> Self {
        Self::obsidian_dark()
    }
}

impl Theme {
    /// Standard Obsidian-inspired dark theme.
    pub fn obsidian_dark() -> Self {
        Self {
            bg_primary: Color::rgb(0.12, 0.12, 0.12),
            bg_secondary: Color::rgb(0.09, 0.09, 0.09),
            bg_tertiary: Color::rgb(0.07, 0.07, 0.07),
            bg_surface: Color::rgb(0.16, 0.16, 0.16),
            border: Color::rgb(0.18, 0.18, 0.18),
            text_primary: Color::rgb(0.86, 0.87, 0.87),
            text_muted: Color::rgb(0.45, 0.46, 0.49),
            accent: Color::rgb(0.48, 0.23, 0.93),
            selection: Color::rgba(0.48, 0.23, 0.93, 0.35),
            cursor: Color::rgb(0.90, 0.90, 0.90),

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
