//! 64x64 tile hash history and swapchain-age damage rectangle coalescing.

use std::num::NonZeroU32;

pub(super) const TILE_SIZE: usize = 64;
const MAX_HISTORY_AGE: usize = 3;

const HASH_SEED: u64 = 0xcbf2_9ce4_8422_2325;
const HASH_PRIME: u64 = 0x0100_0000_01b3;

#[inline]
pub(super) fn hash_init(bg: u32) -> u64 {
    (HASH_SEED ^ bg as u64).wrapping_mul(HASH_PRIME)
}

#[inline]
pub(super) fn hash_mix(h: u64, val: u64) -> u64 {
    (h ^ val).wrapping_mul(HASH_PRIME)
}

#[derive(Clone, Copy)]
pub(super) struct TileMeta {
    pub(super) bg: u32,
    pub(super) first_cmd: u32,
    pub(super) last_cmd: u32,
    pub(super) hash: u64,
}

impl TileMeta {
    #[inline]
    pub(super) fn reset(bg: u32) -> Self {
        Self {
            bg,
            first_cmd: u32::MAX,
            last_cmd: u32::MAX,
            hash: hash_init(bg),
        }
    }
}

pub(super) const MASK_RASTER: u8 = 0b01;
pub(super) const MASK_DAMAGE: u8 = 0b10;

#[derive(Default)]
pub(super) struct DamageTracker {
    width: usize,
    height: usize,
    tiles_x: usize,
    tiles_y: usize,
    valid_ages: u8,
    history: [Vec<u64>; MAX_HISTORY_AGE],
    tile_masks: Vec<u8>,
    bands: Vec<Band>,
    next_bands: Vec<Band>,
    damage_rects: Vec<softbuffer::Rect>,
}

#[derive(Clone, Copy)]
struct Band {
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
}

impl DamageTracker {
    pub(super) fn prepare(
        &mut self,
        width: usize,
        height: usize,
        buffer_age: u8,
        tiles: &[TileMeta],
    ) -> &[u8] {
        let tiles_x = width.div_ceil(TILE_SIZE);
        let tiles_y = height.div_ceil(TILE_SIZE);
        let tile_count = tiles_x * tiles_y;

        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.tiles_x = tiles_x;
            self.tiles_y = tiles_y;
            self.valid_ages = 0;
            for h in &mut self.history {
                h.clear();
                h.resize(tile_count, 0);
            }
        }

        self.tile_masks.clear();
        self.tile_masks.resize(tile_count, 0);
        self.damage_rects.clear();

        let age = buffer_age as usize;
        let can_diff_buffer = age >= 1 && age <= (self.valid_ages as usize);
        let can_diff_surface = self.valid_ages >= 1;

        if !can_diff_buffer {
            self.tile_masks.fill(MASK_RASTER | MASK_DAMAGE);
            if let (Some(w), Some(h)) = (
                NonZeroU32::new(width as u32),
                NonZeroU32::new(height as u32),
            ) {
                self.damage_rects.push(softbuffer::Rect {
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                });
            }
            return &self.tile_masks;
        }

        let buf_ref = &self.history[age - 1];
        let surf_ref = &self.history[0];
        let mut any_surface_damage = false;

        for (i, tile) in tiles.iter().enumerate() {
            let mut m = 0u8;
            if tile.hash != buf_ref[i] {
                m |= MASK_RASTER;
            }
            if !can_diff_surface || tile.hash != surf_ref[i] {
                m |= MASK_DAMAGE;
                any_surface_damage = true;
            }
            self.tile_masks[i] = m;
        }

        // If the scene matches the last presented surface (`history[0]`), nothing on screen
        // changed; clear raster masks too so an unswapped backbuffer is left untouched.
        if !any_surface_damage {
            self.tile_masks.fill(0);
            return &self.tile_masks;
        }

        self.coalesce_damage_rects();
        &self.tile_masks
    }

    /// Commits current tile hashes as the newly presented frame (`age = 1` for next frame).
    pub(super) fn commit_presented(&mut self, tiles: &[TileMeta]) {
        if self.damage_rects.is_empty() {
            return;
        }
        self.history.rotate_right(1);
        let latest = &mut self.history[0];
        latest.resize(tiles.len(), 0);
        for (dst, tile) in latest.iter_mut().zip(tiles.iter()) {
            *dst = tile.hash;
        }
        self.valid_ages = (self.valid_ages + 1).min(MAX_HISTORY_AGE as u8);
    }

    pub(super) fn damage_rects(&self) -> &[softbuffer::Rect] {
        &self.damage_rects
    }

    fn coalesce_damage_rects(&mut self) {
        self.bands.clear();
        let w_u32 = self.width as u32;
        let h_u32 = self.height as u32;

        for ty in 0..self.tiles_y {
            let y0 = (ty * TILE_SIZE) as u32;
            let y1 = ((ty + 1) * TILE_SIZE).min(self.height) as u32;
            let row_base = ty * self.tiles_x;

            self.next_bands.clear();
            let mut tx = 0;
            while tx < self.tiles_x {
                if self.tile_masks[row_base + tx] == 0 {
                    tx += 1;
                    continue;
                }
                let start_tx = tx;
                tx += 1;
                while tx < self.tiles_x && self.tile_masks[row_base + tx] != 0 {
                    tx += 1;
                }
                let x0 = (start_tx * TILE_SIZE) as u32;
                let x1 = (tx * TILE_SIZE).min(self.width) as u32;

                if let Some(prev) = self
                    .bands
                    .iter_mut()
                    .find(|b| b.x0 == x0 && b.x1 == x1 && b.y1 == y0)
                {
                    prev.y1 = y1;
                    self.next_bands.push(*prev);
                    prev.y1 = prev.y0; // Mark merged so it isn't flushed prematurely.
                } else {
                    self.next_bands.push(Band { x0, x1, y0, y1 });
                }
            }

            for finished in self.bands.drain(..) {
                if finished.y1 > finished.y0 && finished.x1 > finished.x0 {
                    push_rect(&mut self.damage_rects, finished, w_u32, h_u32);
                }
            }
            std::mem::swap(&mut self.bands, &mut self.next_bands);
        }

        for remaining in self.bands.drain(..) {
            if remaining.y1 > remaining.y0 && remaining.x1 > remaining.x0 {
                push_rect(&mut self.damage_rects, remaining, w_u32, h_u32);
            }
        }
    }
}

fn push_rect(out: &mut Vec<softbuffer::Rect>, band: Band, max_w: u32, max_h: u32) {
    let x1 = band.x1.min(max_w);
    let y1 = band.y1.min(max_h);
    if let (Some(w), Some(h)) = (
        NonZeroU32::new(x1.saturating_sub(band.x0)),
        NonZeroU32::new(y1.saturating_sub(band.y0)),
    ) {
        out.push(softbuffer::Rect {
            x: band.x0,
            y: band.y0,
            width: w,
            height: h,
        });
    }
}
