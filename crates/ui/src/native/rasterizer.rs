//! Pure-Rust CPU software rasterizer for `egui::ClippedPrimitive` into `0x00RRGGBB` framebuffers.
//!
//! Eliminates OpenGL/EGL/Vulkan dependencies so no GPU driver libraries (`libEGL`,
//! `libnvidia-*`, `libLLVM`, `libgallium`, `/dev/nvidiactl`) are mapped into process RAM.

use std::collections::HashMap;

use egui::epaint::{ImageData, ImageDelta, Mesh, Primitive, Vertex, WHITE_UV};
use egui::{ClippedPrimitive, Color32, Pos2, Rect, TextureFilter, TextureId, TextureOptions};

#[derive(Clone)]
struct CpuTexture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
    options: TextureOptions,
}

impl CpuTexture {
    #[inline]
    fn sample(&self, u: f32, v: f32) -> Color32 {
        if self.width == 0 || self.height == 0 {
            return Color32::WHITE;
        }
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        match self.options.magnification {
            TextureFilter::Nearest => {
                let x = ((u * self.width as f32) as usize).min(self.width - 1);
                let y = ((v * self.height as f32) as usize).min(self.height - 1);
                self.pixels[y * self.width + x]
            }
            TextureFilter::Linear => {
                let fx = (u * self.width as f32 - 0.5).max(0.0);
                let fy = (v * self.height as f32 - 0.5).max(0.0);
                let x0 = (fx as usize).min(self.width - 1);
                let y0 = (fy as usize).min(self.height - 1);
                let x1 = (x0 + 1).min(self.width - 1);
                let y1 = (y0 + 1).min(self.height - 1);

                let tx = ((fx - x0 as f32) * 256.0) as u32;
                let ty = ((fy - y0 as f32) * 256.0) as u32;
                let inv_tx = 256 - tx;
                let inv_ty = 256 - ty;

                let row0 = y0 * self.width;
                let row1 = y1 * self.width;
                let c00 = self.pixels[row0 + x0].to_array();
                let c10 = self.pixels[row0 + x1].to_array();
                let c01 = self.pixels[row1 + x0].to_array();
                let c11 = self.pixels[row1 + x1].to_array();

                let w00 = inv_tx * inv_ty;
                let w10 = tx * inv_ty;
                let w01 = inv_tx * ty;
                let w11 = tx * ty;

                let r = (c00[0] as u32 * w00
                    + c10[0] as u32 * w10
                    + c01[0] as u32 * w01
                    + c11[0] as u32 * w11
                    + 32768)
                    >> 16;
                let g = (c00[1] as u32 * w00
                    + c10[1] as u32 * w10
                    + c01[1] as u32 * w01
                    + c11[1] as u32 * w11
                    + 32768)
                    >> 16;
                let b = (c00[2] as u32 * w00
                    + c10[2] as u32 * w10
                    + c01[2] as u32 * w01
                    + c11[2] as u32 * w11
                    + 32768)
                    >> 16;
                let a = (c00[3] as u32 * w00
                    + c10[3] as u32 * w10
                    + c01[3] as u32 * w01
                    + c11[3] as u32 * w11
                    + 32768)
                    >> 16;

                Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, a as u8)
            }
        }
    }
}

/// CPU software rasterizer maintaining `egui` texture atlases in system RAM.
#[derive(Default)]
pub struct SoftwareRasterizer {
    textures: HashMap<TextureId, CpuTexture>,
}

impl SoftwareRasterizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies and drains texture uploads/patches before rendering the current frame's primitives.
    pub fn set_textures(&mut self, textures_delta: &mut egui::TexturesDelta) {
        for (id, deltas) in textures_delta.set.drain() {
            for delta in deltas {
                self.apply_image_delta(id, delta);
            }
        }
    }

    /// Frees and drains textures retired by `egui` after rendering the current frame's primitives.
    pub fn free_textures(&mut self, textures_delta: &mut egui::TexturesDelta) {
        for id in textures_delta.free.drain() {
            self.textures.remove(&id);
        }
    }

    fn apply_image_delta(&mut self, id: TextureId, delta: ImageDelta) {
        let ImageData::Color(image) = delta.image;
        let [w, h] = image.size;

        if let Some([px, py]) = delta.pos {
            if let Some(tex) = self.textures.get_mut(&id) {
                tex.options = delta.options;
                let max_rows = h.min(tex.height.saturating_sub(py));
                let max_cols = w.min(tex.width.saturating_sub(px));
                for row in 0..max_rows {
                    let src_start = row * w;
                    let dst_start = (py + row) * tex.width + px;
                    tex.pixels[dst_start..dst_start + max_cols]
                        .copy_from_slice(&image.pixels[src_start..src_start + max_cols]);
                }
            }
        } else {
            let color_image = std::sync::Arc::unwrap_or_clone(image);
            self.textures.insert(
                id,
                CpuTexture {
                    width: w,
                    height: h,
                    pixels: color_image.pixels,
                    options: delta.options,
                },
            );
        }
    }

    /// Clears `framebuffer` with `clear_color` and rasterizes all `primitives`.
    ///
    /// `framebuffer` must have length `width * height`, formatted as `0x00RRGGBB`.
    pub fn paint_primitives(
        &self,
        framebuffer: &mut [u32],
        width: usize,
        height: usize,
        pixels_per_point: f32,
        clear_color: Color32,
        primitives: &[ClippedPrimitive],
    ) {
        if width == 0 || height == 0 || framebuffer.len() < width * height {
            return;
        }
        let fb = &mut framebuffer[..width * height];
        fb.fill(pack_rgb(clear_color));

        for ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            let Some(clip) = ClipBounds::from_rect(*clip_rect, pixels_per_point, width, height)
            else {
                continue;
            };
            if let Primitive::Mesh(mesh) = primitive {
                self.paint_mesh(fb, width, pixels_per_point, clip, mesh);
            }
        }
    }

    fn paint_mesh(&self, fb: &mut [u32], fb_width: usize, ppp: f32, clip: ClipBounds, mesh: &Mesh) {
        if mesh.indices.len() < 3 || mesh.vertices.is_empty() {
            return;
        }
        let texture = self.textures.get(&mesh.texture_id);
        let vertices = &mesh.vertices;
        let indices = &mesh.indices;

        let mut cursor = 0;
        while cursor + 3 <= indices.len() {
            if cursor + 6 <= indices.len()
                && self.try_paint_axis_aligned_quad(
                    fb,
                    fb_width,
                    ppp,
                    clip,
                    texture,
                    vertices,
                    &indices[cursor..cursor + 6],
                )
            {
                cursor += 6;
                continue;
            }

            let i0 = indices[cursor] as usize;
            let i1 = indices[cursor + 1] as usize;
            let i2 = indices[cursor + 2] as usize;
            if i0 < vertices.len() && i1 < vertices.len() && i2 < vertices.len() {
                paint_triangle(
                    fb,
                    fb_width,
                    ppp,
                    clip,
                    texture,
                    vertices[i0],
                    vertices[i1],
                    vertices[i2],
                );
            }
            cursor += 3;
        }
    }

    /// Fast path for 2-triangle axis-aligned rectangles (solid UI rects, glyphs, icons).
    #[allow(clippy::too_many_arguments)]
    fn try_paint_axis_aligned_quad(
        &self,
        fb: &mut [u32],
        fb_width: usize,
        ppp: f32,
        clip: ClipBounds,
        texture: Option<&CpuTexture>,
        vertices: &[Vertex],
        idx: &[u32],
    ) -> bool {
        // Both `Mesh::add_rect_with_uv` ([0, 1, 2, 2, 1, 3]) and `fill_closed_path`
        // ([2, 0, 4, 4, 0, 6]) share diagonal vertices at idx[1] == idx[4] and idx[2] == idx[3].
        if idx[1] != idx[4] || idx[2] != idx[3] {
            return false;
        }
        let ia = idx[0] as usize;
        let ib = idx[1] as usize;
        let ic = idx[2] as usize;
        let id = idx[5] as usize;
        if ia >= vertices.len()
            || ib >= vertices.len()
            || ic >= vertices.len()
            || id >= vertices.len()
        {
            return false;
        }

        let va = vertices[ia];
        let vb = vertices[ib];
        let vc = vertices[ic];
        let vd = vertices[id];

        let color = va.color;
        if color != vb.color || color != vc.color || color != vd.color {
            return false;
        }

        // Case 1: Standard `add_rect_with_uv` layout (va = TL, vb = TR, vc = BL, vd = BR).
        let is_standard_uv_quad = (va.pos.y - vb.pos.y).abs() <= 1e-4
            && (vc.pos.y - vd.pos.y).abs() <= 1e-4
            && (va.pos.x - vc.pos.x).abs() <= 1e-4
            && (vb.pos.x - vd.pos.x).abs() <= 1e-4
            && va.pos.x <= vb.pos.x
            && va.pos.y <= vc.pos.y
            && (va.uv.y - vb.uv.y).abs() <= 1e-5
            && (vc.uv.y - vd.uv.y).abs() <= 1e-5
            && (va.uv.x - vc.uv.x).abs() <= 1e-5
            && (vb.uv.x - vd.uv.x).abs() <= 1e-5;

        if is_standard_uv_quad {
            let is_white_uv = is_white(va.uv) && is_white(vd.uv);
            if is_white_uv {
                paint_solid_rect(
                    fb,
                    fb_width,
                    clip,
                    va.pos.x * ppp,
                    va.pos.y * ppp,
                    vd.pos.x * ppp,
                    vd.pos.y * ppp,
                    color,
                );
                return true;
            }
            if let Some(tex) = texture {
                paint_textured_rect(
                    fb,
                    fb_width,
                    clip,
                    va.pos.x * ppp,
                    va.pos.y * ppp,
                    vd.pos.x * ppp,
                    vd.pos.y * ppp,
                    va.uv,
                    vd.uv,
                    color,
                    tex,
                );
                return true;
            }
            return false;
        }

        // Case 2: Untextured axis-aligned rectangle from `fill_closed_path` where
        // (vb, vc) is one diagonal and (va, vd) is the other diagonal.
        if is_white(va.uv) && is_white(vb.uv) && is_white(vc.uv) && is_white(vd.uv) {
            let min_x = vb.pos.x.min(vc.pos.x);
            let max_x = vb.pos.x.max(vc.pos.x);
            let min_y = vb.pos.y.min(vc.pos.y);
            let max_y = vb.pos.y.max(vc.pos.y);

            let other_min_x = va.pos.x.min(vd.pos.x);
            let other_max_x = va.pos.x.max(vd.pos.x);
            let other_min_y = va.pos.y.min(vd.pos.y);
            let other_max_y = va.pos.y.max(vd.pos.y);

            let distinct_diagonals = ((va.pos.x - vb.pos.x).abs() > 1e-4
                || (va.pos.y - vb.pos.y).abs() > 1e-4)
                && ((vd.pos.x - vc.pos.x).abs() > 1e-4 || (vd.pos.y - vc.pos.y).abs() > 1e-4);

            if distinct_diagonals
                && (other_min_x - min_x).abs() <= 1e-4
                && (other_max_x - max_x).abs() <= 1e-4
                && (other_min_y - min_y).abs() <= 1e-4
                && (other_max_y - max_y).abs() <= 1e-4
            {
                paint_solid_rect(
                    fb,
                    fb_width,
                    clip,
                    min_x * ppp,
                    min_y * ppp,
                    max_x * ppp,
                    max_y * ppp,
                    color,
                );
                return true;
            }
        }

        false
    }
}

#[derive(Clone, Copy)]
struct ClipBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl ClipBounds {
    #[inline]
    fn from_rect(rect: Rect, ppp: f32, width: usize, height: usize) -> Option<Self> {
        let min_x = (rect.min.x * ppp).round().max(0.0) as i32;
        let min_y = (rect.min.y * ppp).round().max(0.0) as i32;
        let max_x = (rect.max.x * ppp).round().min(width as f32) as i32;
        let max_y = (rect.max.y * ppp).round().min(height as f32) as i32;
        if min_x < max_x && min_y < max_y {
            Some(Self {
                min_x,
                min_y,
                max_x,
                max_y,
            })
        } else {
            None
        }
    }
}

#[inline(always)]
fn is_white(uv: Pos2) -> bool {
    uv.x == WHITE_UV.x && uv.y == WHITE_UV.y
}

#[inline(always)]
fn pack_rgb(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
}

#[inline(always)]
fn div255(x: u32) -> u32 {
    let t = x + 128;
    (t + (t >> 8)) >> 8
}

#[inline(always)]
fn mul_color(a: Color32, b: Color32) -> Color32 {
    if a == Color32::WHITE {
        return b;
    }
    if b == Color32::WHITE {
        return a;
    }
    Color32::from_rgba_premultiplied(
        div255(a.r() as u32 * b.r() as u32) as u8,
        div255(a.g() as u32 * b.g() as u32) as u8,
        div255(a.b() as u32 * b.b() as u32) as u8,
        div255(a.a() as u32 * b.a() as u32) as u8,
    )
}

#[inline(always)]
fn blend_premultiplied(dst: u32, src: Color32) -> u32 {
    let sa = src.a() as u32;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return pack_rgb(src);
    }
    let inv_a = 255 - sa;
    let dr = (dst >> 16) & 0xFF;
    let dg = (dst >> 8) & 0xFF;
    let db = dst & 0xFF;

    let r = (src.r() as u32 + div255(dr * inv_a)).min(255);
    let g = (src.g() as u32 + div255(dg * inv_a)).min(255);
    let b = (src.b() as u32 + div255(db * inv_a)).min(255);
    (r << 16) | (g << 8) | b
}

#[allow(clippy::too_many_arguments)]
fn paint_solid_rect(
    fb: &mut [u32],
    fb_width: usize,
    clip: ClipBounds,
    px0: f32,
    py0: f32,
    px1: f32,
    py1: f32,
    color: Color32,
) {
    let sa = color.a();
    if sa == 0 {
        return;
    }

    // Top-left rule at pixel centers (x + 0.5, y + 0.5):
    let x0 = ((px0 - 0.5).ceil() as i32).max(clip.min_x);
    let y0 = ((py0 - 0.5).ceil() as i32).max(clip.min_y);
    let x1 = ((px1 - 0.5).ceil() as i32).min(clip.max_x);
    let y1 = ((py1 - 0.5).ceil() as i32).min(clip.max_y);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x0 = x0 as usize;
    let x1 = x1 as usize;
    if sa == 255 {
        let packed = pack_rgb(color);
        for y in y0 as usize..y1 as usize {
            let row_start = y * fb_width;
            fb[row_start + x0..row_start + x1].fill(packed);
        }
    } else {
        for y in y0 as usize..y1 as usize {
            let row_start = y * fb_width;
            for dst in &mut fb[row_start + x0..row_start + x1] {
                *dst = blend_premultiplied(*dst, color);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_textured_rect(
    fb: &mut [u32],
    fb_width: usize,
    clip: ClipBounds,
    px0: f32,
    py0: f32,
    px1: f32,
    py1: f32,
    uv0: Pos2,
    uv1: Pos2,
    tint: Color32,
    tex: &CpuTexture,
) {
    if tint.a() == 0 || tex.width == 0 || tex.height == 0 {
        return;
    }

    let raw_x0 = (px0 - 0.5).ceil() as i32;
    let raw_y0 = (py0 - 0.5).ceil() as i32;
    let raw_x1 = (px1 - 0.5).ceil() as i32;
    let raw_y1 = (py1 - 0.5).ceil() as i32;

    let x0 = raw_x0.max(clip.min_x);
    let y0 = raw_y0.max(clip.min_y);
    let x1 = raw_x1.min(clip.max_x);
    let y1 = raw_y1.min(clip.max_y);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let tw = tex.width as f32;
    let th = tex.height as f32;
    let tx0_f = uv0.x * tw;
    let ty0_f = uv0.y * th;
    let tx1_f = uv1.x * tw;
    let ty1_f = uv1.y * th;

    let tx0 = tx0_f.round() as i32;
    let ty0 = ty0_f.round() as i32;
    let tx1 = tx1_f.round() as i32;
    let ty1 = ty1_f.round() as i32;

    // 1:1 integer-aligned glyph/icon fast path: avoids per-pixel float interpolation
    // and keeps unscaled glyphs in the font atlas pixel-exact.
    let is_one_to_one = (px0 - raw_x0 as f32).abs() <= 1e-3
        && (py0 - raw_y0 as f32).abs() <= 1e-3
        && (tx0_f - tx0 as f32).abs() <= 1e-2
        && (ty0_f - ty0 as f32).abs() <= 1e-2
        && (raw_x1 - raw_x0) == (tx1 - tx0)
        && (raw_y1 - raw_y0) == (ty1 - ty0)
        && tx0 >= 0
        && ty0 >= 0
        && (tx1 as usize) <= tex.width
        && (ty1 as usize) <= tex.height;

    if is_one_to_one {
        let skip_x = (x0 - raw_x0) as usize;
        let skip_y = (y0 - raw_y0) as usize;
        let span_w = (x1 - x0) as usize;
        let span_h = (y1 - y0) as usize;

        for dy in 0..span_h {
            let fb_row = (y0 as usize + dy) * fb_width + x0 as usize;
            let tex_row = (ty0 as usize + skip_y + dy) * tex.width + (tx0 as usize + skip_x);
            let dst_slice = &mut fb[fb_row..fb_row + span_w];
            let src_slice = &tex.pixels[tex_row..tex_row + span_w];
            for (dst, &texel) in dst_slice.iter_mut().zip(src_slice.iter()) {
                if texel.a() != 0 {
                    let src = mul_color(tint, texel);
                    *dst = blend_premultiplied(*dst, src);
                }
            }
        }
        return;
    }

    let inv_w = 1.0 / (px1 - px0).max(1e-6);
    let inv_h = 1.0 / (py1 - py0).max(1e-6);
    let du = uv1.x - uv0.x;
    let dv = uv1.y - uv0.y;

    for y in y0..y1 {
        let ty = ((y as f32 + 0.5) - py0) * inv_h;
        let v = uv0.y + ty * dv;
        let row_offset = y as usize * fb_width;
        for x in x0..x1 {
            let tx = ((x as f32 + 0.5) - px0) * inv_w;
            let u = uv0.x + tx * du;
            let texel = tex.sample(u, v);
            if texel.a() != 0 {
                let src = mul_color(tint, texel);
                let idx = row_offset + x as usize;
                fb[idx] = blend_premultiplied(fb[idx], src);
            }
        }
    }
}

/// 28.4 fixed-point half-space triangle rasterizer with top-left fill rule.
#[allow(clippy::too_many_arguments)]
fn paint_triangle(
    fb: &mut [u32],
    fb_width: usize,
    ppp: f32,
    clip: ClipBounds,
    texture: Option<&CpuTexture>,
    v0: Vertex,
    mut v1: Vertex,
    mut v2: Vertex,
) {
    if v0.color.a() == 0 && v1.color.a() == 0 && v2.color.a() == 0 {
        return;
    }

    let x0 = (v0.pos.x * ppp * 16.0).round() as i64;
    let y0 = (v0.pos.y * ppp * 16.0).round() as i64;
    let mut x1 = (v1.pos.x * ppp * 16.0).round() as i64;
    let mut y1 = (v1.pos.y * ppp * 16.0).round() as i64;
    let mut x2 = (v2.pos.x * ppp * 16.0).round() as i64;
    let mut y2 = (v2.pos.y * ppp * 16.0).round() as i64;

    let mut area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
    if area == 0 {
        return;
    }
    if area < 0 {
        std::mem::swap(&mut v1, &mut v2);
        std::mem::swap(&mut x1, &mut x2);
        std::mem::swap(&mut y1, &mut y2);
        area = -area;
    }

    let min_x = ((x0.min(x1).min(x2) + 7) >> 4).max(clip.min_x as i64) as i32;
    let max_x = ((x0.max(x1).max(x2) + 8) >> 4).min(clip.max_x as i64) as i32;
    let min_y = ((y0.min(y1).min(y2) + 7) >> 4).max(clip.min_y as i64) as i32;
    let max_y = ((y0.max(y1).max(y2) + 8) >> 4).min(clip.max_y as i64) as i32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    // Edge 0: v1 -> v2 (weight for v0)
    let dx0 = x2 - x1;
    let dy0 = y2 - y1;
    let bias0 = if dy0 < 0 || (dy0 == 0 && dx0 > 0) {
        0
    } else {
        -1
    };

    // Edge 1: v2 -> v0 (weight for v1)
    let dx1 = x0 - x2;
    let dy1 = y0 - y2;
    let bias1 = if dy1 < 0 || (dy1 == 0 && dx1 > 0) {
        0
    } else {
        -1
    };

    // Edge 2: v0 -> v1 (weight for v2)
    let dx2 = x1 - x0;
    let dy2 = y1 - y0;
    let bias2 = if dy2 < 0 || (dy2 == 0 && dx2 > 0) {
        0
    } else {
        -1
    };

    let start_sx = (min_x as i64) * 16 + 8;
    let start_sy = (min_y as i64) * 16 + 8;

    let mut row_e0 = dx0 * (start_sy - y1) - dy0 * (start_sx - x1) + bias0;
    let mut row_e1 = dx1 * (start_sy - y2) - dy1 * (start_sx - x2) + bias1;
    let mut row_e2 = dx2 * (start_sy - y0) - dy2 * (start_sx - x0) + bias2;

    let step0_x = -16 * dy0;
    let step1_x = -16 * dy1;
    let step2_x = -16 * dy2;

    let step0_y = 16 * dx0;
    let step1_y = 16 * dx1;
    let step2_y = 16 * dx2;

    let uniform_color = v0.color == v1.color && v0.color == v2.color;
    let untextured = is_white(v0.uv) && is_white(v1.uv) && is_white(v2.uv);

    if uniform_color && untextured {
        let color = v0.color;
        for y in min_y..max_y {
            let mut e0 = row_e0;
            let mut e1 = row_e1;
            let mut e2 = row_e2;
            let row_offset = y as usize * fb_width;
            for x in min_x..max_x {
                if (e0 | e1 | e2) >= 0 {
                    let idx = row_offset + x as usize;
                    fb[idx] = blend_premultiplied(fb[idx], color);
                }
                e0 += step0_x;
                e1 += step1_x;
                e2 += step2_x;
            }
            row_e0 += step0_y;
            row_e1 += step1_y;
            row_e2 += step2_y;
        }
        return;
    }

    let inv_area = 1.0 / (area as f32);
    let c0 = v0.color.to_array();
    let c1 = v1.color.to_array();
    let c2 = v2.color.to_array();

    for y in min_y..max_y {
        let mut e0 = row_e0;
        let mut e1 = row_e1;
        let mut e2 = row_e2;
        let row_offset = y as usize * fb_width;
        for x in min_x..max_x {
            if (e0 | e1 | e2) >= 0 {
                let w0 = ((e0 - bias0) as f32 * inv_area).clamp(0.0, 1.0);
                let w1 = ((e1 - bias1) as f32 * inv_area).clamp(0.0, 1.0);
                let w2 = (1.0 - w0 - w1).clamp(0.0, 1.0);

                let vert_color = if uniform_color {
                    v0.color
                } else {
                    let r = (c0[0] as f32 * w0 + c1[0] as f32 * w1 + c2[0] as f32 * w2 + 0.5) as u8;
                    let g = (c0[1] as f32 * w0 + c1[1] as f32 * w1 + c2[1] as f32 * w2 + 0.5) as u8;
                    let b = (c0[2] as f32 * w0 + c1[2] as f32 * w1 + c2[2] as f32 * w2 + 0.5) as u8;
                    let a = (c0[3] as f32 * w0 + c1[3] as f32 * w1 + c2[3] as f32 * w2 + 0.5) as u8;
                    Color32::from_rgba_premultiplied(r, g, b, a)
                };

                let src = if untextured {
                    vert_color
                } else if let Some(tex) = texture {
                    let u = v0.uv.x * w0 + v1.uv.x * w1 + v2.uv.x * w2;
                    let v = v0.uv.y * w0 + v1.uv.y * w1 + v2.uv.y * w2;
                    mul_color(vert_color, tex.sample(u, v))
                } else {
                    vert_color
                };

                if src.a() != 0 {
                    let idx = row_offset + x as usize;
                    fb[idx] = blend_premultiplied(fb[idx], src);
                }
            }
            e0 += step0_x;
            e1 += step1_x;
            e2 += step2_x;
        }
        row_e0 += step0_y;
        row_e1 += step1_y;
        row_e2 += step2_y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::ColorImage;

    #[test]
    fn solid_rect_fills_clipped_region() {
        let rasterizer = SoftwareRasterizer::new();
        let mut fb = vec![0u32; 16 * 16];
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_max(Pos2::new(2.0, 3.0), Pos2::new(10.0, 12.0)),
            Color32::from_rgb(0x12, 0x34, 0x56),
        );
        let primitives = vec![ClippedPrimitive {
            clip_rect: Rect::from_min_max(Pos2::new(4.0, 5.0), Pos2::new(8.0, 9.0)),
            primitive: Primitive::Mesh(mesh),
        }];
        rasterizer.paint_primitives(&mut fb, 16, 16, 1.0, Color32::BLACK, &primitives);

        for y in 0..16 {
            for x in 0..16 {
                let expected = if (4..8).contains(&x) && (5..9).contains(&y) {
                    0x0012_3456
                } else {
                    0x0000_0000
                };
                assert_eq!(fb[y * 16 + x], expected, "mismatch at ({x}, {y})");
            }
        }
    }

    #[test]
    fn textured_glyph_quad_renders_one_to_one_and_frees() {
        let mut rasterizer = SoftwareRasterizer::new();
        let tex_id = TextureId::Managed(0);
        let mut img = ColorImage::filled([4, 4], Color32::TRANSPARENT);
        img[(1, 1)] = Color32::WHITE;
        img[(2, 2)] = Color32::from_white_alpha(128);
        let mut delta = egui::TexturesDelta::default();
        delta.set.insert(
            tex_id,
            vec![ImageDelta::full(img, TextureOptions::NEAREST)].into(),
        );
        rasterizer.set_textures(&mut delta);

        let mut fb = vec![0u32; 8 * 8];
        let mut mesh = Mesh::with_texture(tex_id);
        mesh.add_rect_with_uv(
            Rect::from_min_max(Pos2::new(2.0, 2.0), Pos2::new(6.0, 6.0)),
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::from_rgb(200, 100, 50),
        );
        let primitives = vec![ClippedPrimitive {
            clip_rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(8.0, 8.0)),
            primitive: Primitive::Mesh(mesh),
        }];
        rasterizer.paint_primitives(&mut fb, 8, 8, 1.0, Color32::BLACK, &primitives);

        assert_eq!(fb[3 * 8 + 3], (200 << 16) | (100 << 8) | 50);
        assert_ne!(fb[4 * 8 + 4], 0);
        assert_eq!(fb[2 * 8 + 2], 0);

        delta.free.insert(tex_id);
        rasterizer.free_textures(&mut delta);
        assert!(!rasterizer.textures.contains_key(&tex_id));
    }

    #[test]
    fn shared_triangle_edge_blends_exactly_once() {
        let rasterizer = SoftwareRasterizer::new();
        let mut fb = vec![0u32; 10 * 10];
        let semi = Color32::from_rgba_unmultiplied(200, 200, 200, 128);

        // Two non-axis-alignedUV triangles sharing a diagonal so they take the general triangle path.
        let mut mesh = Mesh::default();
        mesh.vertices.extend_from_slice(&[
            Vertex {
                pos: Pos2::new(1.0, 1.0),
                uv: WHITE_UV,
                color: semi,
            },
            Vertex {
                pos: Pos2::new(9.0, 1.0),
                uv: WHITE_UV,
                color: semi,
            },
            Vertex {
                pos: Pos2::new(1.0, 9.0),
                uv: WHITE_UV,
                color: semi,
            },
            Vertex {
                pos: Pos2::new(9.0, 9.0),
                uv: WHITE_UV,
                color: semi,
            },
        ]);
        // Non-standard index order so general triangle rasterizer is exercised on both triangles:
        mesh.indices.extend_from_slice(&[0, 1, 2, 3, 2, 1]);

        let primitives = vec![ClippedPrimitive {
            clip_rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)),
            primitive: Primitive::Mesh(mesh),
        }];
        rasterizer.paint_primitives(&mut fb, 10, 10, 1.0, Color32::BLACK, &primitives);

        let expected = fb[2 * 10 + 6];
        assert_ne!(expected, 0);
        for y in 1..9 {
            for x in 1..9 {
                assert_eq!(
                    fb[y * 10 + x],
                    expected,
                    "pixel ({x}, {y}) on or near shared diagonal was double-blended or missed"
                );
            }
        }
    }
}
