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
}

/// Visual theme with colors following Obsidian's sleek dark palette.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Main editor background (#1e1e1e).
    pub bg_primary: Color,
    /// Sidebar / navigation background (#181818).
    pub bg_secondary: Color,
    /// Right panel / statusline background (#141414).
    pub bg_tertiary: Color,
    /// Panel border color (#2d2d2d).
    pub border: Color,
    /// Primary text color (#dcddde).
    pub text_primary: Color,
    /// Muted text for line numbers and paths (#72757d).
    pub text_muted: Color,
    /// Accent color for selection and active tabs (#7c3aed / purple).
    pub accent: Color,
    /// Cursor color (#e0e0e0).
    pub cursor: Color,
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
            border: Color::rgb(0.18, 0.18, 0.18),
            text_primary: Color::rgb(0.86, 0.87, 0.87),
            text_muted: Color::rgb(0.45, 0.46, 0.49),
            accent: Color::rgb(0.48, 0.23, 0.93),
            cursor: Color::rgb(0.90, 0.90, 0.90),
        }
    }
}
