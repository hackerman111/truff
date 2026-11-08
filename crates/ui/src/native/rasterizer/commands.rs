//! `DrawCmd` representation, content fingerprinting, and exact triangle Y-band tile bounds.

use egui::epaint::Vertex;
use egui::{Color32, Pos2, Rect, TextureId};

use super::damage::hash_mix;
use super::texture::pack_rgb;

#[derive(Clone, Copy)]
pub(super) struct ClipBounds {
    pub(super) min_x: i32,
    pub(super) min_y: i32,
    pub(super) max_x: i32,
    pub(super) max_y: i32,
}

impl ClipBounds {
    #[inline]
    pub(super) fn from_rect(rect: Rect, ppp: f32, width: usize, height: usize) -> Option<Self> {
        let min_x = (rect.min.x * ppp).round().max(0.0) as i32;
        let min_y = (rect.min.y * ppp).round().max(0.0) as i32;
        let max_x = (rect.max.x * ppp).round().min(width as f32) as i32;
        let max_y = (rect.max.y * ppp).round().min(height as f32) as i32;
        (min_x < max_x && min_y < max_y).then_some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    #[inline]
    pub(super) fn intersect(self, other: Self) -> Option<Self> {
        let min_x = self.min_x.max(other.min_x);
        let min_y = self.min_y.max(other.min_y);
        let max_x = self.max_x.min(other.max_x);
        let max_y = self.max_y.min(other.max_y);
        (min_x < max_x && min_y < max_y).then_some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }
}

#[derive(Clone, Copy)]
pub(super) enum DrawCmd {
    SolidRect {
        bounds: ClipBounds,
        color: Color32,
    },
    VGradRect {
        bounds: ClipBounds,
        fy0: i64,
        fy1: i64,
        top: Color32,
        bot: Color32,
    },
    HGradRect {
        bounds: ClipBounds,
        fx0: i64,
        fx1: i64,
        left: Color32,
        right: Color32,
    },
    TexturedRect {
        bounds: ClipBounds,
        raw_x0: i32,
        raw_y0: i32,
        raw_x1: i32,
        raw_y1: i32,
        px0: f32,
        py0: f32,
        px1: f32,
        py1: f32,
        uv0: Pos2,
        uv1: Pos2,
        tint: Color32,
        tex_id: TextureId,
        tex_gen: u32,
    },
    SolidTri {
        bounds: ClipBounds,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        x2: i64,
        y2: i64,
        color: Color32,
    },
    GeneralTri {
        bounds: ClipBounds,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        x2: i64,
        y2: i64,
        area: i64,
        v0: Vertex,
        v1: Vertex,
        v2: Vertex,
        tex_id: TextureId,
        tex_gen: u32,
        untextured: bool,
        uniform_color: bool,
    },
}

impl DrawCmd {
    #[inline]
    pub(super) fn bounds(&self) -> ClipBounds {
        match *self {
            Self::SolidRect { bounds, .. }
            | Self::VGradRect { bounds, .. }
            | Self::HGradRect { bounds, .. }
            | Self::TexturedRect { bounds, .. }
            | Self::SolidTri { bounds, .. }
            | Self::GeneralTri { bounds, .. } => bounds,
        }
    }

    #[inline]
    pub(super) fn opaque_fill(&self) -> Option<(ClipBounds, u32)> {
        if let Self::SolidRect { bounds, color } = *self
            && color.a() == 255
        {
            Some((bounds, pack_rgb(color)))
        } else {
            None
        }
    }

    /// Exact pixel X range `[min_x..max_x)` intersected by this command across pixel rows `[y0..y1)`.
    ///
    /// Prevents diagonal sliver triangles (such as `epaint` rounded-rectangle corner fans) from
    /// binning into dozens of tiles across their bounding box that the triangle never touches.
    #[inline]
    pub(super) fn x_bounds_for_y_band(&self, y0: i32, y1: i32) -> Option<(i32, i32)> {
        match *self {
            Self::SolidRect { bounds, .. }
            | Self::VGradRect { bounds, .. }
            | Self::HGradRect { bounds, .. }
            | Self::TexturedRect { bounds, .. } => Some((bounds.min_x, bounds.max_x)),
            Self::SolidTri {
                bounds,
                x0: vx0,
                y0: vy0,
                x1: vx1,
                y1: vy1,
                x2: vx2,
                y2: vy2,
                ..
            }
            | Self::GeneralTri {
                bounds,
                x0: vx0,
                y0: vy0,
                x1: vx1,
                y1: vy1,
                x2: vx2,
                y2: vy2,
                ..
            } => tri_x_bounds_for_y_band(bounds, vx0, vy0, vx1, vy1, vx2, vy2, y0, y1),
        }
    }

    /// Standalone content fingerprint computed once per command before tile binning.
    #[inline]
    pub(super) fn cmd_hash(&self) -> u64 {
        const SEED: u64 = 0x517c_c1b7_2722_0a95;
        match *self {
            Self::SolidRect { bounds, color } => {
                let h = hash_bounds(hash_mix(SEED, 1), bounds);
                hash_mix(h, u32::from_le_bytes(color.to_array()) as u64)
            }
            Self::VGradRect {
                bounds,
                fy0,
                fy1,
                top,
                bot,
            } => {
                let mut h = hash_bounds(hash_mix(SEED, 2), bounds);
                h = hash_mix(h, ((fy0 as u64) << 32) ^ (fy1 as u32 as u64));
                let c = ((u32::from_le_bytes(top.to_array()) as u64) << 32)
                    | (u32::from_le_bytes(bot.to_array()) as u64);
                hash_mix(h, c)
            }
            Self::HGradRect {
                bounds,
                fx0,
                fx1,
                left,
                right,
            } => {
                let mut h = hash_bounds(hash_mix(SEED, 3), bounds);
                h = hash_mix(h, ((fx0 as u64) << 32) ^ (fx1 as u32 as u64));
                let c = ((u32::from_le_bytes(left.to_array()) as u64) << 32)
                    | (u32::from_le_bytes(right.to_array()) as u64);
                hash_mix(h, c)
            }
            Self::TexturedRect {
                bounds,
                px0,
                py0,
                px1,
                py1,
                uv0,
                uv1,
                tint,
                tex_id,
                tex_gen,
                ..
            } => {
                let mut h = hash_bounds(hash_mix(SEED, 4), bounds);
                h = hash_mix(h, ((px0.to_bits() as u64) << 32) | (py0.to_bits() as u64));
                h = hash_mix(h, ((px1.to_bits() as u64) << 32) | (py1.to_bits() as u64));
                h = hash_mix(
                    h,
                    ((uv0.x.to_bits() as u64) << 32) | (uv0.y.to_bits() as u64),
                );
                h = hash_mix(
                    h,
                    ((uv1.x.to_bits() as u64) << 32) | (uv1.y.to_bits() as u64),
                );
                h = hash_mix(
                    h,
                    ((u32::from_le_bytes(tint.to_array()) as u64) << 32) | tex_gen as u64,
                );
                hash_tex_id(h, tex_id)
            }
            Self::SolidTri {
                bounds,
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                color,
            } => {
                let mut h = hash_bounds(hash_mix(SEED, 5), bounds);
                h = hash_mix(h, ((x0 as u64) << 32) ^ (y0 as u32 as u64));
                h = hash_mix(h, ((x1 as u64) << 32) ^ (y1 as u32 as u64));
                h = hash_mix(h, ((x2 as u64) << 32) ^ (y2 as u32 as u64));
                hash_mix(h, u32::from_le_bytes(color.to_array()) as u64)
            }
            Self::GeneralTri {
                bounds,
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                v0,
                v1,
                v2,
                tex_id,
                tex_gen,
                ..
            } => {
                let mut h = hash_bounds(hash_mix(SEED, 6), bounds);
                h = hash_mix(h, ((x0 as u64) << 32) ^ (y0 as u32 as u64));
                h = hash_mix(h, ((x1 as u64) << 32) ^ (y1 as u32 as u64));
                h = hash_mix(h, ((x2 as u64) << 32) ^ (y2 as u32 as u64));
                h = hash_vertex_attrs(h, v0);
                h = hash_vertex_attrs(h, v1);
                h = hash_vertex_attrs(h, v2);
                h = hash_mix(h, tex_gen as u64);
                hash_tex_id(h, tex_id)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn tri_x_bounds_for_y_band(
    bounds: ClipBounds,
    vx0: i64,
    vy0: i64,
    vx1: i64,
    vy1: i64,
    vx2: i64,
    vy2: i64,
    y0: i32,
    y1: i32,
) -> Option<(i32, i32)> {
    let band_y0 = y0.max(bounds.min_y);
    let band_y1 = y1.min(bounds.max_y);
    if band_y0 >= band_y1 {
        return None;
    }
    let sy_min = (band_y0 as i64) * 16 + 8;
    let sy_max = ((band_y1 - 1) as i64) * 16 + 8;

    let mut min_sx = i64::MAX;
    let mut max_sx = i64::MIN;

    for (vx, vy) in [(vx0, vy0), (vx1, vy1), (vx2, vy2)] {
        if vy >= sy_min && vy <= sy_max {
            min_sx = min_sx.min(vx);
            max_sx = max_sx.max(vx);
        }
    }

    for (xa, ya, xb, yb) in [
        (vx0, vy0, vx1, vy1),
        (vx1, vy1, vx2, vy2),
        (vx2, vy2, vx0, vy0),
    ] {
        for sy_line in [sy_min, sy_max] {
            if (ya < sy_line && yb > sy_line) || (yb < sy_line && ya > sy_line) {
                let num = (xb - xa) * (sy_line - ya);
                let den = yb - ya;
                let (q, rem) = if den > 0 {
                    (num.div_euclid(den), num.rem_euclid(den))
                } else {
                    ((-num).div_euclid(-den), (-num).rem_euclid(-den))
                };
                let sx_lo = xa + q;
                let sx_hi = sx_lo + i64::from(rem != 0);
                min_sx = min_sx.min(sx_lo);
                max_sx = max_sx.max(sx_hi);
            }
        }
    }

    if min_sx > max_sx {
        return None;
    }

    let min_x = (((min_sx + 7) >> 4) as i32).max(bounds.min_x);
    let max_x = (((max_sx + 8) >> 4) as i32).min(bounds.max_x);
    (min_x < max_x).then_some((min_x, max_x))
}

#[inline]
fn hash_bounds(h: u64, b: ClipBounds) -> u64 {
    let lo = ((b.min_x as u32 as u64) << 32) | (b.min_y as u32 as u64);
    let hi = ((b.max_x as u32 as u64) << 32) | (b.max_y as u32 as u64);
    hash_mix(hash_mix(h, lo), hi)
}

#[inline]
fn hash_tex_id(h: u64, id: TextureId) -> u64 {
    match id {
        TextureId::Managed(n) => hash_mix(h, n),
        TextureId::User(n) => hash_mix(h, n ^ 0x8000_0000_0000_0000),
    }
}

#[inline]
fn hash_vertex_attrs(mut h: u64, v: Vertex) -> u64 {
    h = hash_mix(h, u32::from_le_bytes(v.color.to_array()) as u64);
    hash_mix(
        h,
        ((v.uv.x.to_bits() as u64) << 32) | (v.uv.y.to_bits() as u64),
    )
}

#[inline]
pub(super) fn lerp_color_fixed(c0: Color32, c1: Color32, t_num: i64, t_den: i64) -> Color32 {
    let a0 = c0.to_array();
    let a1 = c1.to_array();
    let t = (t_num as f32 / t_den as f32).clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgba_premultiplied(
        (a0[0] as f32 * inv + a1[0] as f32 * t + 0.5) as u8,
        (a0[1] as f32 * inv + a1[1] as f32 * t + 0.5) as u8,
        (a0[2] as f32 * inv + a1[2] as f32 * t + 0.5) as u8,
        (a0[3] as f32 * inv + a1[3] as f32 * t + 0.5) as u8,
    )
}
