//! Per-tile scanline rasterization kernels for rects, coverage glyphs, and triangles.

use std::collections::HashMap;

use egui::epaint::Vertex;
use egui::{Color32, Pos2, TextureId};

use super::commands::{ClipBounds, DrawCmd, lerp_color_fixed};
use super::texture::{
    CpuTexture, Texels, blend_premultiplied, blend_span_solid, div255, mul_color, pack_rgb,
};

/// Executes `cmd` into a local tile buffer positioned at screen `(tile_x0, tile_y0)` with `stride`.
pub(super) fn execute_cmd_in_tile(
    cmd: &DrawCmd,
    tile_buf: &mut [u32],
    stride: usize,
    tile_x0: i32,
    tile_y0: i32,
    tile_clip: ClipBounds,
    textures: &HashMap<TextureId, CpuTexture>,
) {
    let Some(clip) = cmd.bounds().intersect(tile_clip) else {
        return;
    };

    match *cmd {
        DrawCmd::SolidRect { color, .. } => {
            let rx0 = (clip.min_x - tile_x0) as usize;
            let rx1 = (clip.max_x - tile_x0) as usize;
            for y in clip.min_y..clip.max_y {
                let row = (y - tile_y0) as usize * stride;
                blend_span_solid(&mut tile_buf[row + rx0..row + rx1], color);
            }
        }
        DrawCmd::VGradRect {
            fy0, fy1, top, bot, ..
        } => {
            let rx0 = (clip.min_x - tile_x0) as usize;
            let rx1 = (clip.max_x - tile_x0) as usize;
            let den = fy1 - fy0;
            for y in clip.min_y..clip.max_y {
                let sy = (y as i64) * 16 + 8;
                let row_color = lerp_color_fixed(top, bot, sy - fy0, den);
                if row_color.a() > 0 {
                    let row = (y - tile_y0) as usize * stride;
                    blend_span_solid(&mut tile_buf[row + rx0..row + rx1], row_color);
                }
            }
        }
        DrawCmd::HGradRect {
            fx0,
            fx1,
            left,
            right,
            ..
        } => {
            let den = fx1 - fx0;
            for x in clip.min_x..clip.max_x {
                let sx = (x as i64) * 16 + 8;
                let col_color = lerp_color_fixed(left, right, sx - fx0, den);
                if col_color.a() == 0 {
                    continue;
                }
                let rx = (x - tile_x0) as usize;
                if col_color.a() == 255 {
                    let packed = pack_rgb(col_color);
                    for y in clip.min_y..clip.max_y {
                        tile_buf[(y - tile_y0) as usize * stride + rx] = packed;
                    }
                } else {
                    for y in clip.min_y..clip.max_y {
                        let idx = (y - tile_y0) as usize * stride + rx;
                        tile_buf[idx] = blend_premultiplied(tile_buf[idx], col_color);
                    }
                }
            }
        }
        DrawCmd::TexturedRect {
            raw_x0,
            raw_y0,
            raw_x1,
            raw_y1,
            px0,
            py0,
            px1,
            py1,
            uv0,
            uv1,
            tint,
            tex_id,
            ..
        } => {
            let Some(tex) = textures.get(&tex_id) else {
                return;
            };
            paint_textured_rect_in_tile(
                tile_buf, stride, tile_x0, tile_y0, clip, raw_x0, raw_y0, raw_x1, raw_y1, px0, py0,
                px1, py1, uv0, uv1, tint, tex,
            );
        }
        DrawCmd::SolidTri {
            x0,
            y0,
            x1,
            y1,
            x2,
            y2,
            color,
            ..
        } => {
            paint_solid_tri_in_tile(
                tile_buf, stride, tile_x0, tile_y0, clip, x0, y0, x1, y1, x2, y2, color,
            );
        }
        DrawCmd::GeneralTri {
            x0,
            y0,
            x1,
            y1,
            x2,
            y2,
            area,
            v0,
            v1,
            v2,
            tex_id,
            untextured,
            uniform_color,
            ..
        } => {
            let tex = (!untextured).then(|| textures.get(&tex_id)).flatten();
            paint_general_tri_in_tile(
                tile_buf,
                stride,
                tile_x0,
                tile_y0,
                clip,
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                area,
                v0,
                v1,
                v2,
                tex,
                untextured,
                uniform_color,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_textured_rect_in_tile(
    tile_buf: &mut [u32],
    stride: usize,
    tile_x0: i32,
    tile_y0: i32,
    clip: ClipBounds,
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
    tex: &CpuTexture,
) {
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
        let skip_x = (clip.min_x - raw_x0) as usize;
        let skip_y = (clip.min_y - raw_y0) as usize;
        let span_w = (clip.max_x - clip.min_x) as usize;
        let span_h = (clip.max_y - clip.min_y) as usize;
        let rx0 = (clip.min_x - tile_x0) as usize;
        let ry0 = (clip.min_y - tile_y0) as usize;

        match &tex.texels {
            Texels::Alpha(alphas) => {
                let packed_tint = pack_rgb(tint);
                let tint_opaque = tint.a() == 255;
                let tr = tint.r() as u32;
                let tg = tint.g() as u32;
                let tb = tint.b() as u32;
                let ta = tint.a() as u32;
                for dy in 0..span_h {
                    let fb_row = (ry0 + dy) * stride + rx0;
                    let tex_row =
                        (ty0 as usize + skip_y + dy) * tex.width + (tx0 as usize + skip_x);
                    let dst_slice = &mut tile_buf[fb_row..fb_row + span_w];
                    let src_slice = &alphas[tex_row..tex_row + span_w];
                    for (dst, &cov) in dst_slice.iter_mut().zip(src_slice.iter()) {
                        if cov == 0 {
                            continue;
                        }
                        if cov == 255 && tint_opaque {
                            *dst = packed_tint;
                            continue;
                        }
                        let c = cov as u32;
                        let sa = if tint_opaque { c } else { div255(ta * c) };
                        if sa == 0 {
                            continue;
                        }
                        let sr = div255(tr * c);
                        let sg = div255(tg * c);
                        let sb = div255(tb * c);
                        if sa == 255 {
                            *dst = (sr << 16) | (sg << 8) | sb;
                        } else {
                            let inv_a = 255 - sa;
                            let d = *dst;
                            let r = (sr + div255(((d >> 16) & 0xFF) * inv_a)).min(255);
                            let g = (sg + div255(((d >> 8) & 0xFF) * inv_a)).min(255);
                            let b = (sb + div255((d & 0xFF) * inv_a)).min(255);
                            *dst = (r << 16) | (g << 8) | b;
                        }
                    }
                }
            }
            Texels::Rgba(rgba) => {
                for dy in 0..span_h {
                    let fb_row = (ry0 + dy) * stride + rx0;
                    let tex_row =
                        (ty0 as usize + skip_y + dy) * tex.width + (tx0 as usize + skip_x);
                    let dst_slice = &mut tile_buf[fb_row..fb_row + span_w];
                    let src_slice = &rgba[tex_row..tex_row + span_w];
                    for (dst, &texel) in dst_slice.iter_mut().zip(src_slice.iter()) {
                        if texel.a() != 0 {
                            let src = mul_color(tint, texel);
                            *dst = blend_premultiplied(*dst, src);
                        }
                    }
                }
            }
        }
        return;
    }

    let inv_w = 1.0 / (px1 - px0).max(1e-6);
    let inv_h = 1.0 / (py1 - py0).max(1e-6);
    let du = uv1.x - uv0.x;
    let dv = uv1.y - uv0.y;

    for y in clip.min_y..clip.max_y {
        let ty = ((y as f32 + 0.5) - py0) * inv_h;
        let v = uv0.y + ty * dv;
        let row_offset = (y - tile_y0) as usize * stride;
        for x in clip.min_x..clip.max_x {
            let tx = ((x as f32 + 0.5) - px0) * inv_w;
            let u = uv0.x + tx * du;
            let texel = tex.sample(u, v);
            if texel.a() != 0 {
                let src = mul_color(tint, texel);
                let idx = row_offset + (x - tile_x0) as usize;
                tile_buf[idx] = blend_premultiplied(tile_buf[idx], src);
            }
        }
    }
}

/// Shared 16.4 fixed-point half-space edge stepper with Direct3D/OpenGL top-left fill rules.
struct TriEdgeStepper {
    bias0: i64,
    bias1: i64,
    row_e0: i64,
    row_e1: i64,
    row_e2: i64,
    step0_x: i64,
    step1_x: i64,
    step2_x: i64,
    step0_y: i64,
    step1_y: i64,
    step2_y: i64,
}

impl TriEdgeStepper {
    #[inline]
    fn new(clip: ClipBounds, x0: i64, y0: i64, x1: i64, y1: i64, x2: i64, y2: i64) -> Self {
        let dx0 = x2 - x1;
        let dy0 = y2 - y1;
        let bias0 = if dy0 < 0 || (dy0 == 0 && dx0 > 0) {
            0
        } else {
            -1
        };
        let dx1 = x0 - x2;
        let dy1 = y0 - y2;
        let bias1 = if dy1 < 0 || (dy1 == 0 && dx1 > 0) {
            0
        } else {
            -1
        };
        let dx2 = x1 - x0;
        let dy2 = y1 - y0;
        let bias2 = if dy2 < 0 || (dy2 == 0 && dx2 > 0) {
            0
        } else {
            -1
        };

        let start_sx = (clip.min_x as i64) * 16 + 8;
        let start_sy = (clip.min_y as i64) * 16 + 8;

        Self {
            bias0,
            bias1,
            row_e0: dx0 * (start_sy - y1) - dy0 * (start_sx - x1) + bias0,
            row_e1: dx1 * (start_sy - y2) - dy1 * (start_sx - x2) + bias1,
            row_e2: dx2 * (start_sy - y0) - dy2 * (start_sx - x0) + bias2,
            step0_x: -16 * dy0,
            step1_x: -16 * dy1,
            step2_x: -16 * dy2,
            step0_y: 16 * dx0,
            step1_y: 16 * dx1,
            step2_y: 16 * dx2,
        }
    }

    /// Exact integer scanline span `[x_lo..=x_hi]` where all 3 half-space edge equations are `>= 0`.
    #[inline]
    fn row_span(&self, min_x: i32, max_x: i32) -> (i32, i32) {
        let mut k_lo = 0i64;
        let mut k_hi = (max_x - min_x - 1) as i64;

        for (e, s) in [
            (self.row_e0, self.step0_x),
            (self.row_e1, self.step1_x),
            (self.row_e2, self.step2_x),
        ] {
            if s == 0 {
                if e < 0 {
                    return (1, 0);
                }
            } else if s > 0 {
                if e < 0 {
                    let need = (-e + s - 1) / s;
                    if need > k_lo {
                        k_lo = need;
                    }
                }
            } else if e < 0 {
                return (1, 0);
            } else {
                let max_k = e / (-s);
                if max_k < k_hi {
                    k_hi = max_k;
                }
            }
            if k_lo > k_hi {
                return (1, 0);
            }
        }

        (min_x + k_lo as i32, min_x + k_hi as i32)
    }

    #[inline]
    fn next_row(&mut self) {
        self.row_e0 += self.step0_y;
        self.row_e1 += self.step1_y;
        self.row_e2 += self.step2_y;
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_solid_tri_in_tile(
    tile_buf: &mut [u32],
    stride: usize,
    tile_x0: i32,
    tile_y0: i32,
    clip: ClipBounds,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    color: Color32,
) {
    let mut step = TriEdgeStepper::new(clip, x0, y0, x1, y1, x2, y2);
    for y in clip.min_y..clip.max_y {
        let (x_lo, x_hi) = step.row_span(clip.min_x, clip.max_x);
        if x_lo <= x_hi {
            let row = (y - tile_y0) as usize * stride;
            let rx0 = (x_lo - tile_x0) as usize;
            let rx1 = (x_hi + 1 - tile_x0) as usize;
            blend_span_solid(&mut tile_buf[row + rx0..row + rx1], color);
        }
        step.next_row();
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_general_tri_in_tile(
    tile_buf: &mut [u32],
    stride: usize,
    tile_x0: i32,
    tile_y0: i32,
    clip: ClipBounds,
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
    texture: Option<&CpuTexture>,
    untextured: bool,
    uniform_color: bool,
) {
    let mut step = TriEdgeStepper::new(clip, x0, y0, x1, y1, x2, y2);
    let inv_area = 1.0 / (area as f32);
    let c0 = v0.color.to_array();
    let c1 = v1.color.to_array();
    let c2 = v2.color.to_array();

    for y in clip.min_y..clip.max_y {
        let (x_lo, x_hi) = step.row_span(clip.min_x, clip.max_x);
        if x_lo <= x_hi {
            let skip = (x_lo - clip.min_x) as i64;
            let mut e0 = step.row_e0 + skip * step.step0_x;
            let mut e1 = step.row_e1 + skip * step.step1_x;
            let row_offset = (y - tile_y0) as usize * stride;
            for x in x_lo..=x_hi {
                let w0 = ((e0 - step.bias0) as f32 * inv_area).clamp(0.0, 1.0);
                let w1 = ((e1 - step.bias1) as f32 * inv_area).clamp(0.0, 1.0);
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
                    let idx = row_offset + (x - tile_x0) as usize;
                    tile_buf[idx] = blend_premultiplied(tile_buf[idx], src);
                }
                e0 += step.step0_x;
                e1 += step.step1_x;
            }
        }
        step.next_row();
    }
}
