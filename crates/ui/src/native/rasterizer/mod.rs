//! Pure-Rust CPU tile-binned software rasterizer for `egui::ClippedPrimitive` into `0x00RRGGBB` framebuffers.
//!
//! Eliminates OpenGL/EGL/Vulkan dependencies so no GPU driver libraries (`libEGL`,
//! `libnvidia-*`, `libLLVM`, `libgallium`, `/dev/nvidiactl`) are mapped into process RAM.
//!
//! Architecture:
//! - `texture`: coverage-compressed (`1 B/px`) font/icon atlases with automatic RGBA promotion.
//! - `commands`: `DrawCmd` representation, content hashing, and exact triangle Y-band tile bounds.
//! - `classify`: mesh-to-`DrawCmd` axis-aligned quad/feather extraction and triangle emission.
//! - `kernels`: L1-resident scanline-span rect, coverage glyph, and triangle kernels.
//! - `damage`: `64x64` tile binning with opaque occlusion culling, swapchain `buffer.age()`
//!   hash comparison (`age = 1..=3`), and `softbuffer::Rect` damage coalescing.

mod classify;
mod commands;
mod damage;
mod kernels;
mod texture;

use std::collections::HashMap;

use egui::epaint::{ImageData, ImageDelta, Primitive};
use egui::{ClippedPrimitive, Color32, TextureId};

use self::classify::emit_mesh_commands;
use self::commands::{ClipBounds, DrawCmd};
use self::damage::{DamageTracker, MASK_RASTER, TILE_SIZE, TileMeta, hash_mix};
use self::kernels::execute_cmd_in_tile;
use self::texture::{CpuTexture, pack_rgb};

/// CPU software rasterizer maintaining `egui` texture atlases and tile-damage state in RAM.
#[derive(Default)]
pub struct SoftwareRasterizer {
    textures: HashMap<TextureId, CpuTexture>,
    cmds: Vec<DrawCmd>,
    cmd_links: Vec<(u32, u32)>,
    tiles: Vec<TileMeta>,
    damage: DamageTracker,
    next_tex_gen: u32,
}

impl SoftwareRasterizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total heap capacity (in bytes) currently held by CPU texture atlases.
    pub fn texture_bytes(&self) -> usize {
        self.textures.values().map(CpuTexture::byte_len).sum()
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
        if let Some(pos) = delta.pos {
            if let Some(tex) = self.textures.get_mut(&id) {
                tex.apply_patch(pos, &image, delta.options);
            }
        } else {
            self.next_tex_gen = self.next_tex_gen.wrapping_add(1);
            let tex = CpuTexture::from_color_image(image, delta.options, self.next_tex_gen);
            self.textures.insert(id, tex);
        }
    }

    /// Rasterizes only tiles that changed relative to `buffer_age`, returning coalesced surface damage.
    ///
    /// When the returned slice is empty and `buffer_age > 0`, nothing changed relative to the last
    /// presented frame and the caller can skip `present` (unless handling an uncomposited Expose).
    #[allow(clippy::too_many_arguments)]
    pub fn paint_frame(
        &mut self,
        framebuffer: &mut [u32],
        width: usize,
        height: usize,
        pixels_per_point: f32,
        clear_color: Color32,
        primitives: &[ClippedPrimitive],
        buffer_age: u8,
    ) -> &[softbuffer::Rect] {
        if width == 0 || height == 0 || framebuffer.len() < width * height {
            return &[];
        }
        let fb = &mut framebuffer[..width * height];
        self.bin_primitives(width, height, pixels_per_point, clear_color, primitives);

        let masks = self.damage.prepare(width, height, buffer_age, &self.tiles);
        let tiles_x = width.div_ceil(TILE_SIZE);
        let tiles_y = height.div_ceil(TILE_SIZE);

        let mut tile_buf = [0u32; TILE_SIZE * TILE_SIZE];
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let tile_idx = ty * tiles_x + tx;
                if (masks[tile_idx] & MASK_RASTER) == 0 {
                    continue;
                }
                rasterize_tile(
                    &self.tiles[tile_idx],
                    &self.cmds,
                    &self.cmd_links,
                    &self.textures,
                    &mut tile_buf,
                    fb,
                    width,
                    height,
                    tx,
                    ty,
                );
            }
        }

        self.damage.commit_presented(&self.tiles);
        self.damage.damage_rects()
    }

    /// Stateless full-framebuffer rasterization (`&self`), clearing `framebuffer` with `clear_color`.
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
        let mut cmds = Vec::new();
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
                emit_mesh_commands(mesh, pixels_per_point, clip, &self.textures, &mut cmds);
            }
        }

        let tiles_x = width.div_ceil(TILE_SIZE);
        let tiles_y = height.div_ceil(TILE_SIZE);
        let mut tiles = vec![TileMeta::reset(pack_rgb(clear_color)); tiles_x * tiles_y];
        let mut cmd_links = Vec::with_capacity(cmds.len() * 2);
        bin_cmd_slice(&cmds, width, height, tiles_x, &mut tiles, &mut cmd_links);

        let mut tile_buf = [0u32; TILE_SIZE * TILE_SIZE];
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                rasterize_tile(
                    &tiles[ty * tiles_x + tx],
                    &cmds,
                    &cmd_links,
                    &self.textures,
                    &mut tile_buf,
                    fb,
                    width,
                    height,
                    tx,
                    ty,
                );
            }
        }
    }

    fn bin_primitives(
        &mut self,
        width: usize,
        height: usize,
        ppp: f32,
        clear_color: Color32,
        primitives: &[ClippedPrimitive],
    ) {
        self.cmds.clear();
        for ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            let Some(clip) = ClipBounds::from_rect(*clip_rect, ppp, width, height) else {
                continue;
            };
            if let Primitive::Mesh(mesh) = primitive {
                emit_mesh_commands(mesh, ppp, clip, &self.textures, &mut self.cmds);
            }
        }

        let tiles_x = width.div_ceil(TILE_SIZE);
        let tiles_y = height.div_ceil(TILE_SIZE);
        let bg = pack_rgb(clear_color);
        self.tiles.clear();
        self.tiles.resize(tiles_x * tiles_y, TileMeta::reset(bg));
        self.cmd_links.clear();

        bin_cmd_slice(
            &self.cmds,
            width,
            height,
            tiles_x,
            &mut self.tiles,
            &mut self.cmd_links,
        );
    }
}

fn bin_cmd_slice(
    cmds: &[DrawCmd],
    width: usize,
    height: usize,
    tiles_x: usize,
    tiles: &mut [TileMeta],
    cmd_links: &mut Vec<(u32, u32)>,
) {
    let ts = TILE_SIZE as i32;
    let max_tx = tiles_x.saturating_sub(1) as i32;
    let max_ty = height.div_ceil(TILE_SIZE).saturating_sub(1) as i32;

    for (cmd_idx, cmd) in cmds.iter().enumerate() {
        let b = cmd.bounds();
        let ty0 = (b.min_y / ts).clamp(0, max_ty) as usize;
        let ty1 = ((b.max_y - 1) / ts).clamp(0, max_ty) as usize;
        let opaque = cmd.opaque_fill();
        let cmd_hash = cmd.cmd_hash();

        for ty in ty0..=ty1 {
            let tile_y0 = (ty * TILE_SIZE) as i32;
            let tile_y1 = ((ty + 1) * TILE_SIZE).min(height) as i32;
            let Some((band_min_x, band_max_x)) = (if ty0 == ty1 {
                Some((b.min_x, b.max_x))
            } else {
                cmd.x_bounds_for_y_band(tile_y0, tile_y1)
            }) else {
                continue;
            };
            let tx0 = (band_min_x / ts).clamp(0, max_tx) as usize;
            let tx1 = ((band_max_x - 1) / ts).clamp(0, max_tx) as usize;
            let row_off = ty * tiles_x;

            for tx in tx0..=tx1 {
                let tile_x0 = (tx * TILE_SIZE) as i32;
                let tile_x1 = ((tx + 1) * TILE_SIZE).min(width) as i32;
                let tile = &mut tiles[row_off + tx];

                // Opaque occlusion culling: a solid opaque rect covering the entire tile
                // replaces `tile.bg` and discards all earlier occluded commands on this tile.
                if let Some((ob, packed_rgb)) = opaque
                    && ob.min_x <= tile_x0
                    && ob.max_x >= tile_x1
                    && ob.min_y <= tile_y0
                    && ob.max_y >= tile_y1
                {
                    *tile = TileMeta::reset(packed_rgb);
                    continue;
                }

                let link_idx = cmd_links.len() as u32;
                cmd_links.push((cmd_idx as u32, u32::MAX));
                if tile.last_cmd == u32::MAX {
                    tile.first_cmd = link_idx;
                } else {
                    cmd_links[tile.last_cmd as usize].1 = link_idx;
                }
                tile.last_cmd = link_idx;
                tile.hash = hash_mix(tile.hash, cmd_hash);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_tile(
    tile: &TileMeta,
    cmds: &[DrawCmd],
    cmd_links: &[(u32, u32)],
    textures: &HashMap<TextureId, CpuTexture>,
    tile_buf: &mut [u32; TILE_SIZE * TILE_SIZE],
    fb: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    tx: usize,
    ty: usize,
) {
    let x0 = tx * TILE_SIZE;
    let y0 = ty * TILE_SIZE;
    let tw = TILE_SIZE.min(fb_width - x0);
    let th = TILE_SIZE.min(fb_height - y0);

    // Fast path: tile has no surviving draw commands (pure solid background/panel color).
    if tile.first_cmd == u32::MAX {
        for dy in 0..th {
            let row_start = (y0 + dy) * fb_width + x0;
            fb[row_start..row_start + tw].fill(tile.bg);
        }
        return;
    }

    let local = &mut tile_buf[..tw * th];
    local.fill(tile.bg);

    let tile_clip = ClipBounds {
        min_x: x0 as i32,
        min_y: y0 as i32,
        max_x: (x0 + tw) as i32,
        max_y: (y0 + th) as i32,
    };

    let mut cur = tile.first_cmd;
    while cur != u32::MAX {
        let (cmd_idx, next) = cmd_links[cur as usize];
        execute_cmd_in_tile(
            &cmds[cmd_idx as usize],
            local,
            tw,
            x0 as i32,
            y0 as i32,
            tile_clip,
            textures,
        );
        cur = next;
    }

    for dy in 0..th {
        let dst_start = (y0 + dy) * fb_width + x0;
        let src_start = dy * tw;
        fb[dst_start..dst_start + tw].copy_from_slice(&local[src_start..src_start + tw]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    use egui::{ColorImage, Pos2, Rect, TextureOptions};

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
    fn monochrome_font_atlas_uses_one_byte_per_texel_and_promotes_on_color_patch() {
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
        assert_eq!(rasterizer.texture_bytes(), 16);

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

        // Patching with non-monochrome color promotes `Alpha` -> `Rgba` seamlessly.
        let mut color_patch = ColorImage::filled([1, 1], Color32::from_rgb(10, 200, 30));
        color_patch[(0, 0)] = Color32::from_rgb(10, 200, 30);
        delta.set.insert(
            tex_id,
            vec![ImageDelta::partial(
                [0, 0],
                color_patch,
                TextureOptions::NEAREST,
            )]
            .into(),
        );
        rasterizer.set_textures(&mut delta);
        assert_eq!(rasterizer.texture_bytes(), 64);

        delta.free.insert(tex_id);
        rasterizer.free_textures(&mut delta);
        assert_eq!(rasterizer.texture_bytes(), 0);
    }

    #[test]
    fn shared_triangle_edge_blends_exactly_once() {
        let rasterizer = SoftwareRasterizer::new();
        let mut fb = vec![0u32; 12 * 12];
        let semi = Color32::from_rgba_unmultiplied(200, 200, 200, 128);

        // Skewed parallelogram so both triangles exercise `SolidTri` scanline half-space rules
        // across a shared diagonal without axis-aligned quad collapsing.
        let mut mesh = Mesh::default();
        mesh.vertices.extend_from_slice(&[
            Vertex {
                pos: Pos2::new(2.0, 1.0),
                uv: WHITE_UV,
                color: semi,
            },
            Vertex {
                pos: Pos2::new(10.0, 1.0),
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
        mesh.indices.extend_from_slice(&[0, 1, 2, 3, 2, 1]);

        let primitives = vec![ClippedPrimitive {
            clip_rect: Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(12.0, 12.0)),
            primitive: Primitive::Mesh(mesh),
        }];
        rasterizer.paint_primitives(&mut fb, 12, 12, 1.0, Color32::BLACK, &primitives);

        let expected = fb[4 * 12 + 5];
        assert_ne!(expected, 0);
        for y in 2..8 {
            for x in 3..8 {
                assert_eq!(
                    fb[y * 12 + x],
                    expected,
                    "pixel ({x}, {y}) on or near shared diagonal was double-blended or missed"
                );
            }
        }
    }

    #[test]
    fn swapchain_tile_damage_matches_full_redraw_for_single_and_double_buffering() {
        let w = 200;
        let h = 150;
        let clip = Rect::from_min_max(Pos2::ZERO, Pos2::new(w as f32, h as f32));
        let build_scene = |cursor_y: f32| -> Vec<ClippedPrimitive> {
            let mut mesh = Mesh::default();
            // Full background + editor panel (tests opaque tile occlusion culling).
            mesh.add_colored_rect(clip, Color32::from_rgb(30, 34, 42));
            mesh.add_colored_rect(
                Rect::from_min_max(Pos2::new(40.0, 10.0), Pos2::new(190.0, 140.0)),
                Color32::from_rgb(46, 52, 64),
            );
            // Cursor across tile boundary (64px) + semi-transparent highlight.
            mesh.add_colored_rect(
                Rect::from_min_max(Pos2::new(50.0, cursor_y), Pos2::new(52.0, cursor_y + 18.0)),
                Color32::from_rgb(136, 192, 208),
            );
            mesh.add_colored_rect(
                Rect::from_min_max(Pos2::new(52.0, cursor_y), Pos2::new(150.0, cursor_y + 18.0)),
                Color32::from_rgba_unmultiplied(136, 192, 208, 48),
            );
            vec![ClippedPrimitive {
                clip_rect: clip,
                primitive: Primitive::Mesh(mesh),
            }]
        };

        // Test double-buffered Wayland swapchain (`buf_a`, `buf_b`), including `A -> B -> A` blink!
        let mut rast = SoftwareRasterizer::new();
        let ref_rast = SoftwareRasterizer::new();
        let mut buf_a = vec![0u32; w * h];
        let mut buf_b = vec![0u32; w * h];
        let mut age_a = 0u8;
        let mut age_b = 0u8;
        let mut expected = vec![0u32; w * h];

        for (step, y) in [20.0, 20.0, 56.0, 20.0, 90.0, 90.0].into_iter().enumerate() {
            let scene = build_scene(y);
            ref_rast.paint_primitives(&mut expected, w, h, 1.0, Color32::BLACK, &scene);

            let damage = rast.paint_frame(&mut buf_a, w, h, 1.0, Color32::BLACK, &scene, age_a);
            if !damage.is_empty() || age_a == 0 {
                assert_eq!(buf_a, expected, "double-buffer mismatch at step {step}");
                std::mem::swap(&mut buf_a, &mut buf_b);
                std::mem::swap(&mut age_a, &mut age_b);
                age_b = 1;
                if age_a != 0 {
                    age_a = age_a.saturating_add(1);
                }
            } else {
                // Idle frame (`damage.is_empty()`): front buffer (`buf_b`) already equals `expected`!
                assert_eq!(buf_b, expected, "idle front-buffer mismatch at step {step}");
            }
        }
    }
}
