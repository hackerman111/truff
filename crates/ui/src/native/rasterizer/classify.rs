//! Mesh-to-`DrawCmd` classification: axis-aligned solid/feathered/textured quads and triangles.

use std::collections::HashMap;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Pos2, TextureId};

use super::commands::{ClipBounds, DrawCmd, lerp_color_fixed};
use super::texture::CpuTexture;

#[inline]
fn is_white(uv: Pos2) -> bool {
    uv.x == WHITE_UV.x && uv.y == WHITE_UV.y
}

/// Appends visible `DrawCmd`s for `mesh` into `out`.
pub(super) fn emit_mesh_commands(
    mesh: &Mesh,
    ppp: f32,
    clip: ClipBounds,
    textures: &HashMap<TextureId, CpuTexture>,
    out: &mut Vec<DrawCmd>,
) {
    if mesh.indices.len() < 3 || mesh.vertices.is_empty() {
        return;
    }
    let tex_entry = textures.get(&mesh.texture_id);
    let tex_gen = tex_entry.map_or(0, |t| t.generation);
    let vertices = &mesh.vertices;
    let indices = &mesh.indices;

    let mut cursor = 0;
    while cursor + 3 <= indices.len() {
        if cursor + 6 <= indices.len()
            && try_emit_quad(
                vertices,
                &indices[cursor..cursor + 6],
                ppp,
                clip,
                mesh.texture_id,
                tex_entry,
                tex_gen,
                out,
            )
        {
            cursor += 6;
            continue;
        }

        let i0 = indices[cursor] as usize;
        let i1 = indices[cursor + 1] as usize;
        let i2 = indices[cursor + 2] as usize;
        if i0 < vertices.len() && i1 < vertices.len() && i2 < vertices.len() {
            emit_triangle(
                vertices[i0],
                vertices[i1],
                vertices[i2],
                ppp,
                clip,
                mesh.texture_id,
                tex_gen,
                out,
            );
        }
        cursor += 3;
    }
}

#[allow(clippy::too_many_arguments)]
fn try_emit_quad(
    vertices: &[Vertex],
    idx: &[u32],
    ppp: f32,
    clip: ClipBounds,
    tex_id: TextureId,
    tex_entry: Option<&CpuTexture>,
    tex_gen: u32,
    out: &mut Vec<DrawCmd>,
) -> bool {
    let Some([tl, tr, bl, br]) = extract_axis_aligned_quad(vertices, idx) else {
        return false;
    };

    let all_white = is_white(tl.uv) && is_white(tr.uv) && is_white(bl.uv) && is_white(br.uv);
    let uniform_color = tl.color == tr.color && tl.color == bl.color && tl.color == br.color;

    if all_white {
        let fx0 = (tl.pos.x * ppp * 16.0).round() as i64;
        let fy0 = (tl.pos.y * ppp * 16.0).round() as i64;
        let fx1 = (br.pos.x * ppp * 16.0).round() as i64;
        let fy1 = (br.pos.y * ppp * 16.0).round() as i64;
        let x0 = (((fx0 + 7) >> 4) as i32).max(clip.min_x);
        let y0 = (((fy0 + 7) >> 4) as i32).max(clip.min_y);
        let x1 = (((fx1 + 7) >> 4) as i32).min(clip.max_x);
        let y1 = (((fy1 + 7) >> 4) as i32).min(clip.max_y);

        if uniform_color {
            if tl.color.a() > 0 && x0 < x1 && y0 < y1 {
                out.push(DrawCmd::SolidRect {
                    bounds: ClipBounds {
                        min_x: x0,
                        min_y: y0,
                        max_x: x1,
                        max_y: y1,
                    },
                    color: tl.color,
                });
            }
            return true;
        }

        if tl.color == tr.color && bl.color == br.color {
            if x0 >= x1 || y0 >= y1 || fy1 <= fy0 || (tl.color.a() == 0 && bl.color.a() == 0) {
                return true;
            }
            if y1 - y0 == 1 {
                let sy = (y0 as i64) * 16 + 8;
                let row_color = lerp_color_fixed(tl.color, bl.color, sy - fy0, fy1 - fy0);
                if row_color.a() > 0 {
                    out.push(DrawCmd::SolidRect {
                        bounds: ClipBounds {
                            min_x: x0,
                            min_y: y0,
                            max_x: x1,
                            max_y: y1,
                        },
                        color: row_color,
                    });
                }
                return true;
            }
            out.push(DrawCmd::VGradRect {
                bounds: ClipBounds {
                    min_x: x0,
                    min_y: y0,
                    max_x: x1,
                    max_y: y1,
                },
                fy0,
                fy1,
                top: tl.color,
                bot: bl.color,
            });
            return true;
        }

        if tl.color == bl.color && tr.color == br.color {
            if x0 >= x1 || y0 >= y1 || fx1 <= fx0 || (tl.color.a() == 0 && tr.color.a() == 0) {
                return true;
            }
            if x1 - x0 == 1 {
                let sx = (x0 as i64) * 16 + 8;
                let col_color = lerp_color_fixed(tl.color, tr.color, sx - fx0, fx1 - fx0);
                if col_color.a() > 0 {
                    out.push(DrawCmd::SolidRect {
                        bounds: ClipBounds {
                            min_x: x0,
                            min_y: y0,
                            max_x: x1,
                            max_y: y1,
                        },
                        color: col_color,
                    });
                }
                return true;
            }
            out.push(DrawCmd::HGradRect {
                bounds: ClipBounds {
                    min_x: x0,
                    min_y: y0,
                    max_x: x1,
                    max_y: y1,
                },
                fx0,
                fx1,
                left: tl.color,
                right: tr.color,
            });
            return true;
        }

        return false;
    }

    if uniform_color
        && (tl.uv.y - tr.uv.y).abs() <= 1e-5
        && (bl.uv.y - br.uv.y).abs() <= 1e-5
        && (tl.uv.x - bl.uv.x).abs() <= 1e-5
        && (tr.uv.x - br.uv.x).abs() <= 1e-5
        && let Some(tex) = tex_entry
    {
        if tl.color.a() == 0 || tex.width == 0 || tex.height == 0 {
            return true;
        }
        let px0 = tl.pos.x * ppp;
        let py0 = tl.pos.y * ppp;
        let px1 = br.pos.x * ppp;
        let py1 = br.pos.y * ppp;
        let raw_x0 = (px0 - 0.5).ceil() as i32;
        let raw_y0 = (py0 - 0.5).ceil() as i32;
        let raw_x1 = (px1 - 0.5).ceil() as i32;
        let raw_y1 = (py1 - 0.5).ceil() as i32;
        let x0 = raw_x0.max(clip.min_x);
        let y0 = raw_y0.max(clip.min_y);
        let x1 = raw_x1.min(clip.max_x);
        let y1 = raw_y1.min(clip.max_y);
        if x0 < x1 && y0 < y1 {
            out.push(DrawCmd::TexturedRect {
                bounds: ClipBounds {
                    min_x: x0,
                    min_y: y0,
                    max_x: x1,
                    max_y: y1,
                },
                raw_x0,
                raw_y0,
                raw_x1,
                raw_y1,
                px0,
                py0,
                px1,
                py1,
                uv0: tl.uv,
                uv1: br.uv,
                tint: tl.color,
                tex_id,
                tex_gen,
            });
        }
        return true;
    }

    false
}

/// Recognizes any 2-triangle quad `[a0, a1, a2, b0, b1, b2]` whose shared edge is a diagonal
/// of a non-degenerate axis-aligned rectangle, returning `[tl, tr, bl, br]`.
fn extract_axis_aligned_quad(vertices: &[Vertex], idx: &[u32]) -> Option<[Vertex; 4]> {
    let a = [idx[0] as usize, idx[1] as usize, idx[2] as usize];
    let b = [idx[3] as usize, idx[4] as usize, idx[5] as usize];
    let len = vertices.len();
    if a[0] >= len || a[1] >= len || a[2] >= len || b[0] >= len || b[1] >= len || b[2] >= len {
        return None;
    }

    // Fast path for standard `epaint` quad layout `[i0, i1, i2, i0, i2, i3]`.
    if a[0] == b[0] && a[2] == b[1] && a[1] != b[2] {
        let v0 = vertices[a[0]];
        let v1 = vertices[a[1]];
        let v2 = vertices[a[2]];
        let v3 = vertices[b[2]];
        if (v0.pos.x - v3.pos.x).abs() <= 1e-4
            && (v1.pos.x - v2.pos.x).abs() <= 1e-4
            && (v0.pos.y - v1.pos.y).abs() <= 1e-4
            && (v3.pos.y - v2.pos.y).abs() <= 1e-4
            && v1.pos.x - v0.pos.x > 1e-4
            && v2.pos.y - v0.pos.y > 1e-4
        {
            return Some([v0, v1, v3, v2]);
        }
    }

    let mut shared = [0usize; 2];
    let mut shared_n = 0;
    let mut unshared_a = 0usize;
    let mut unshared_a_n = 0;
    for &ia in &a {
        if b.contains(&ia) {
            if shared_n == 2 {
                return None;
            }
            shared[shared_n] = ia;
            shared_n += 1;
        } else {
            unshared_a = ia;
            unshared_a_n += 1;
        }
    }
    if shared_n != 2 || unshared_a_n != 1 || shared[0] == shared[1] {
        return None;
    }
    let mut unshared_b = 0usize;
    let mut unshared_b_n = 0;
    for &ib in &b {
        if ib != shared[0] && ib != shared[1] {
            unshared_b = ib;
            unshared_b_n += 1;
        }
    }
    if unshared_b_n != 1 {
        return None;
    }

    let vs0 = vertices[shared[0]];
    let vs1 = vertices[shared[1]];
    let vu0 = vertices[unshared_a];
    let vu1 = vertices[unshared_b];

    let min_x = vs0.pos.x.min(vs1.pos.x);
    let max_x = vs0.pos.x.max(vs1.pos.x);
    let min_y = vs0.pos.y.min(vs1.pos.y);
    let max_y = vs0.pos.y.max(vs1.pos.y);

    if max_x - min_x <= 1e-4
        || max_y - min_y <= 1e-4
        || (vu0.pos.x.min(vu1.pos.x) - min_x).abs() > 1e-4
        || (vu0.pos.x.max(vu1.pos.x) - max_x).abs() > 1e-4
        || (vu0.pos.y.min(vu1.pos.y) - min_y).abs() > 1e-4
        || (vu0.pos.y.max(vu1.pos.y) - max_y).abs() > 1e-4
    {
        return None;
    }

    let quad = [vs0, vs1, vu0, vu1];
    let find_corner = |tx: f32, ty: f32| -> Option<Vertex> {
        quad.into_iter()
            .find(|v| (v.pos.x - tx).abs() <= 1e-4 && (v.pos.y - ty).abs() <= 1e-4)
    };
    Some([
        find_corner(min_x, min_y)?,
        find_corner(max_x, min_y)?,
        find_corner(min_x, max_y)?,
        find_corner(max_x, max_y)?,
    ])
}

#[allow(clippy::too_many_arguments)]
fn emit_triangle(
    v0: Vertex,
    mut v1: Vertex,
    mut v2: Vertex,
    ppp: f32,
    clip: ClipBounds,
    tex_id: TextureId,
    tex_gen: u32,
    out: &mut Vec<DrawCmd>,
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

    let min_x = (((x0.min(x1).min(x2) + 7) >> 4) as i32).max(clip.min_x);
    let max_x = (((x0.max(x1).max(x2) + 8) >> 4) as i32).min(clip.max_x);
    let min_y = (((y0.min(y1).min(y2) + 7) >> 4) as i32).max(clip.min_y);
    let max_y = (((y0.max(y1).max(y2) + 8) >> 4) as i32).min(clip.max_y);
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let bounds = ClipBounds {
        min_x,
        min_y,
        max_x,
        max_y,
    };

    let uniform_color = v0.color == v1.color && v0.color == v2.color;
    let untextured = is_white(v0.uv) && is_white(v1.uv) && is_white(v2.uv);
    if uniform_color && untextured {
        out.push(DrawCmd::SolidTri {
            bounds,
            x0,
            y0,
            x1,
            y1,
            x2,
            y2,
            color: v0.color,
        });
    } else {
        out.push(DrawCmd::GeneralTri {
            bounds,
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
            tex_gen,
            untextured,
            uniform_color,
        });
    }
}
