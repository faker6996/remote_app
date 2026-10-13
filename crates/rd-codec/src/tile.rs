use rd_core::domain::models::ScreenFrame;

/// Represents a single dirty tile that changed on screen
#[derive(Debug, Clone)]
pub struct DirtyTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub bgra_data: Vec<u8>,
}

/// Manages tile-based change detection across consecutive frames
pub struct TileManager {
    tile_size: u32,
    prev_frame: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

impl TileManager {
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size: tile_size.max(32),
            prev_frame: None,
            width: 0,
            height: 0,
        }
    }

    /// Compares current frame with previous frame and returns dirty tiles if changes are localized.
    /// Returns:
    /// - None if whole frame needs to be refreshed (first frame, resolution changed, or > 30% tiles dirty)
    /// - Some(Vec<DirtyTile>) with the specific dirty tiles that changed (or empty vec if identical)
    pub fn detect_dirty_tiles(&mut self, frame: &ScreenFrame) -> Option<Vec<DirtyTile>> {
        let w = frame.width;
        let h = frame.height;

        // If first frame or resolution changed, update baseline and trigger full frame
        if self.prev_frame.is_none() || self.width != w || self.height != h {
            self.width = w;
            self.height = h;
            self.prev_frame = Some(frame.data.clone());
            return None;
        }

        let prev = self.prev_frame.as_ref().unwrap();
        if prev.len() != frame.data.len() {
            self.prev_frame = Some(frame.data.clone());
            return None;
        }

        let tile_s = self.tile_size;
        let tiles_x = (w + tile_s - 1) / tile_s;
        let tiles_y = (h + tile_s - 1) / tile_s;
        let total_tiles = tiles_x * tiles_y;

        let mut dirty_tiles = Vec::new();
        let stride = (w * 4) as usize;

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let x = tx * tile_s;
                let y = ty * tile_s;
                let tw = tile_s.min(w - x);
                let th = tile_s.min(h - y);

                let mut is_tile_dirty = false;
                let tile_row_bytes = (tw * 4) as usize;

                // Compare each row within this tile
                for row in 0..th {
                    let cur_offset = ((y + row) as usize * stride) + (x as usize * 4);
                    let prev_offset = cur_offset;

                    if cur_offset + tile_row_bytes <= frame.data.len() && prev_offset + tile_row_bytes <= prev.len() {
                        if frame.data[cur_offset..cur_offset + tile_row_bytes] != prev[prev_offset..prev_offset + tile_row_bytes] {
                            is_tile_dirty = true;
                            break;
                        }
                    }
                }

                if is_tile_dirty {
                    // Extract tile BGRA pixels
                    let mut tile_data = Vec::with_capacity((tw * th * 4) as usize);
                    for row in 0..th {
                        let cur_offset = ((y + row) as usize * stride) + (x as usize * 4);
                        tile_data.extend_from_slice(&frame.data[cur_offset..cur_offset + tile_row_bytes]);
                    }

                    dirty_tiles.push(DirtyTile {
                        x,
                        y,
                        width: tw,
                        height: th,
                        bgra_data: tile_data,
                    });
                }
            }
        }

        // If more than 30% of tiles are dirty, full frame is more efficient
        let max_dirty_threshold = (total_tiles * 3) / 10;
        if dirty_tiles.is_empty() {
            Some(Vec::new())
        } else if dirty_tiles.len() as u32 > max_dirty_threshold {
            self.prev_frame = Some(frame.data.clone());
            None
        } else {
            // Localized changes: update dirty tiles in stored baseline
            if let Some(ref mut p) = self.prev_frame {
                for tile in &dirty_tiles {
                    let tw_bytes = (tile.width * 4) as usize;
                    for row in 0..tile.height {
                        let offset = ((tile.y + row) as usize * stride) + (tile.x as usize * 4);
                        let tile_offset = row as usize * tw_bytes;
                        p[offset..offset + tw_bytes].copy_from_slice(&tile.bgra_data[tile_offset..tile_offset + tw_bytes]);
                    }
                }
            }
            Some(dirty_tiles)
        }
    }
}
