//! CPU texture storage (`Alpha` monochrome atlas vs premultiplied `Rgba`) and pixel blending.

use std::sync::Arc;

use egui::{Color32, ColorImage, TextureFilter, TextureOptions};

#[derive(Clone)]
pub(super) enum Texels {
    /// Coverage-only texels where premultiplied `r == g == b == a` (e.g. `epaint` font atlas).
    /// Uses 1 byte/texel instead of 4 bytes/texel so glyph atlases stay in L2 cache.
    Alpha(Vec<u8>),
    /// Arbitrary premultiplied RGBA texels.
    Rgba(Vec<Color32>),
}

#[derive(Clone)]
pub(super) struct CpuTexture {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) texels: Texels,
    pub(super) options: TextureOptions,
    pub(super) generation: u32,
}

impl CpuTexture {
    pub(super) fn from_color_image(
        image: Arc<ColorImage>,
        options: TextureOptions,
        generation: u32,
    ) -> Self {
        let [width, height] = image.size;
        if is_monochrome(&image.pixels) {
            let alphas = image.pixels.iter().map(|c| c.a()).collect();
            Self {
                width,
                height,
                texels: Texels::Alpha(alphas),
                options,
                generation,
            }
        } else {
            let owned = Arc::unwrap_or_clone(image);
            Self {
                width,
                height,
                texels: Texels::Rgba(owned.pixels),
                options,
                generation,
            }
        }
    }

    pub(super) fn apply_patch(
        &mut self,
        [px, py]: [usize; 2],
        image: &ColorImage,
        options: TextureOptions,
    ) {
        if self.options != options {
            self.options = options;
            self.generation = self.generation.wrapping_add(1);
        }
        let [w, h] = image.size;
        let max_rows = h.min(self.height.saturating_sub(py));
        let max_cols = w.min(self.width.saturating_sub(px));
        if max_rows == 0 || max_cols == 0 {
            return;
        }

        let patch_mono = is_monochrome(&image.pixels);
        if !patch_mono && let Texels::Alpha(alphas) = &self.texels {
            let promoted = alphas
                .iter()
                .copied()
                .map(Color32::from_white_alpha)
                .collect();
            self.texels = Texels::Rgba(promoted);
        }

        let mut mutated_existing = false;
        match &mut self.texels {
            Texels::Alpha(alphas) => {
                for row in 0..max_rows {
                    let src_start = row * w;
                    let dst_start = (py + row) * self.width + px;
                    for col in 0..max_cols {
                        let new_a = image.pixels[src_start + col].a();
                        let dst = &mut alphas[dst_start + col];
                        if *dst != 0 && *dst != new_a {
                            mutated_existing = true;
                        }
                        *dst = new_a;
                    }
                }
            }
            Texels::Rgba(rgba) => {
                for row in 0..max_rows {
                    let src_start = row * w;
                    let dst_start = (py + row) * self.width + px;
                    for col in 0..max_cols {
                        let new_c = image.pixels[src_start + col];
                        let dst = &mut rgba[dst_start + col];
                        if dst.a() != 0 && *dst != new_c {
                            mutated_existing = true;
                        }
                        *dst = new_c;
                    }
                }
            }
        }
        if mutated_existing {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub(super) fn byte_len(&self) -> usize {
        match &self.texels {
            Texels::Alpha(v) => v.capacity(),
            Texels::Rgba(v) => v.capacity() * std::mem::size_of::<Color32>(),
        }
    }

    #[inline]
    pub(super) fn sample(&self, u: f32, v: f32) -> Color32 {
        if self.width == 0 || self.height == 0 {
            return Color32::WHITE;
        }
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        match (&self.texels, self.options.magnification) {
            (Texels::Alpha(alphas), TextureFilter::Nearest) => {
                let x = ((u * self.width as f32) as usize).min(self.width - 1);
                let y = ((v * self.height as f32) as usize).min(self.height - 1);
                Color32::from_white_alpha(alphas[y * self.width + x])
            }
            (Texels::Rgba(rgba), TextureFilter::Nearest) => {
                let x = ((u * self.width as f32) as usize).min(self.width - 1);
                let y = ((v * self.height as f32) as usize).min(self.height - 1);
                rgba[y * self.width + x]
            }
            (Texels::Alpha(alphas), TextureFilter::Linear) => {
                let (row0, row1, x0, x1, w00, w10, w01, w11) = self.linear_tap_weights(u, v);
                let a = (alphas[row0 + x0] as u32 * w00
                    + alphas[row0 + x1] as u32 * w10
                    + alphas[row1 + x0] as u32 * w01
                    + alphas[row1 + x1] as u32 * w11
                    + 32768)
                    >> 16;
                Color32::from_white_alpha(a as u8)
            }
            (Texels::Rgba(rgba), TextureFilter::Linear) => {
                let (row0, row1, x0, x1, w00, w10, w01, w11) = self.linear_tap_weights(u, v);
                let c00 = rgba[row0 + x0].to_array();
                let c10 = rgba[row0 + x1].to_array();
                let c01 = rgba[row1 + x0].to_array();
                let c11 = rgba[row1 + x1].to_array();
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

    #[inline]
    fn linear_tap_weights(
        &self,
        u: f32,
        v: f32,
    ) -> (usize, usize, usize, usize, u32, u32, u32, u32) {
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
        (
            y0 * self.width,
            y1 * self.width,
            x0,
            x1,
            inv_tx * inv_ty,
            tx * inv_ty,
            inv_tx * ty,
            tx * ty,
        )
    }
}

fn is_monochrome(pixels: &[Color32]) -> bool {
    pixels.iter().all(|c| {
        let [r, g, b, a] = c.to_array();
        r == a && g == a && b == a
    })
}

#[inline]
pub(super) fn pack_rgb(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32)
}

#[inline]
pub(super) fn div255(x: u32) -> u32 {
    let t = x + 128;
    (t + (t >> 8)) >> 8
}

#[inline]
pub(super) fn mul_color(a: Color32, b: Color32) -> Color32 {
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

#[inline]
pub(super) fn blend_premultiplied(dst: u32, src: Color32) -> u32 {
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

#[inline]
pub(super) fn blend_span_solid(slice: &mut [u32], color: Color32) {
    let sa = color.a() as u32;
    if sa == 0 || slice.is_empty() {
        return;
    }
    if sa == 255 {
        slice.fill(pack_rgb(color));
        return;
    }
    let inv_a = 255 - sa;
    let sr = color.r() as u32;
    let sg = color.g() as u32;
    let sb = color.b() as u32;
    for dst in slice {
        let d = *dst;
        let r = (sr + div255(((d >> 16) & 0xFF) * inv_a)).min(255);
        let g = (sg + div255(((d >> 8) & 0xFF) * inv_a)).min(255);
        let b = (sb + div255((d & 0xFF) * inv_a)).min(255);
        *dst = (r << 16) | (g << 8) | b;
    }
}
