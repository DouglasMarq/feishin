use super::hashing::scramble;

const RING_SHADE: u32 = 40;
const OPAQUE: u8 = 255;

struct CoverPalette {
    red: u32,
    green: u32,
    blue: u32,
}

impl CoverPalette {
    fn for_cover(cover_art_id: &str) -> Self {
        let seed = cover_art_id.bytes().fold(0u64, |accumulated, byte| {
            scramble(accumulated ^ u64::from(byte))
        });
        Self {
            red: (seed & 0xff) as u32,
            green: ((seed >> 8) & 0xff) as u32,
            blue: ((seed >> 16) & 0xff) as u32,
        }
    }

    fn pixel(&self, column: u32, row: u32, size: u32) -> [u8; 4] {
        let blend = (column + row) * 255 / (2 * size);
        let shade = ring_shade(column, row, size);
        [
            blend_channel(self.red, self.blue, blend, shade),
            blend_channel(self.green, self.red, blend, shade),
            blend_channel(self.blue, self.green, blend, shade),
            OPAQUE,
        ]
    }
}

fn ring_shade(column: u32, row: u32, size: u32) -> u32 {
    let center = size as i64 / 2;
    let squared_distance = (column as i64 - center).pow(2) + (row as i64 - center).pow(2);
    let ring = squared_distance.unsigned_abs() * 4 / u64::from(size * size / 4).max(1);
    u32::from(ring.is_multiple_of(2)) * RING_SHADE
}

fn blend_channel(from: u32, to: u32, blend: u32, shade: u32) -> u8 {
    ((from * (255 - blend) + to * blend) / 255 + shade).min(255) as u8
}

pub fn render_cover(cover_art_id: &str, size: u32) -> Vec<u8> {
    let palette = CoverPalette::for_cover(cover_art_id);
    let size = size.max(1);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for column in 0..size {
            rgba.extend_from_slice(&palette.pixel(column, row, size));
        }
    }
    rgba
}
