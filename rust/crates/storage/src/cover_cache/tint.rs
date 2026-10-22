use super::DecodedImage;

const RGBA_BYTES_PER_PIXEL: usize = 4;
const SAMPLES_PER_EDGE: usize = 48;
const HUE_BUCKETS: usize = 12;
const DEGREES_PER_BUCKET: f32 = 360.0 / HUE_BUCKETS as f32;
const MIN_OPAQUE_ALPHA: u8 = 128;
const MIN_BRIGHTNESS: u8 = 32;
const MIN_CHROMA: u8 = 28;
const MIN_COLORFUL_SHARE_DIVISOR: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Copy, Default)]
struct HueBucket {
    weight: f32,
    red: f32,
    green: f32,
    blue: f32,
}

impl HueBucket {
    fn add(&mut self, pixel: Rgb, weight: f32) {
        self.weight += weight;
        self.red += f32::from(pixel.red) * weight;
        self.green += f32::from(pixel.green) * weight;
        self.blue += f32::from(pixel.blue) * weight;
    }

    fn average(&self) -> Option<Rgb> {
        let channel = |sum: f32| (sum / self.weight).round().clamp(0.0, 255.0) as u8;
        (self.weight > 0.0).then(|| Rgb {
            red: channel(self.red),
            green: channel(self.green),
            blue: channel(self.blue),
        })
    }
}

impl DecodedImage {
    pub fn dominant_tint(&self) -> Option<Rgb> {
        let width = self.width as usize;
        let height = self.height as usize;
        let pixel_count = width.checked_mul(height)?;
        if pixel_count == 0 || self.rgba.len() != pixel_count * RGBA_BYTES_PER_PIXEL {
            return None;
        }
        let step = (width.max(height) / SAMPLES_PER_EDGE).max(1);
        let mut buckets = [HueBucket::default(); HUE_BUCKETS];
        let mut sampled = 0_usize;
        let mut colorful = 0_usize;
        for row in (0..height).step_by(step) {
            for column in (0..width).step_by(step) {
                sampled += 1;
                let Some((pixel, chroma)) = self.colorful_pixel(row * width + column) else {
                    continue;
                };
                colorful += 1;
                if let Some(bucket) = buckets.get_mut(hue_bucket(pixel, chroma)) {
                    bucket.add(pixel, f32::from(chroma));
                }
            }
        }
        if colorful == 0 || colorful < sampled / MIN_COLORFUL_SHARE_DIVISOR {
            return None;
        }
        buckets
            .iter()
            .max_by(|first, second| first.weight.total_cmp(&second.weight))?
            .average()
    }

    fn colorful_pixel(&self, pixel_index: usize) -> Option<(Rgb, u8)> {
        let start = pixel_index * RGBA_BYTES_PER_PIXEL;
        let [red, green, blue, alpha] = *self.rgba.get(start..start + RGBA_BYTES_PER_PIXEL)? else {
            return None;
        };
        let brightest = red.max(green).max(blue);
        let chroma = brightest - red.min(green).min(blue);
        let is_colorful =
            alpha >= MIN_OPAQUE_ALPHA && brightest >= MIN_BRIGHTNESS && chroma >= MIN_CHROMA;
        is_colorful.then_some((Rgb { red, green, blue }, chroma))
    }
}

fn hue_bucket(pixel: Rgb, chroma: u8) -> usize {
    let red = f32::from(pixel.red);
    let green = f32::from(pixel.green);
    let blue = f32::from(pixel.blue);
    let chroma = f32::from(chroma);
    let brightest = red.max(green).max(blue);
    let sector = if brightest == red {
        ((green - blue) / chroma).rem_euclid(6.0)
    } else if brightest == green {
        (blue - red) / chroma + 2.0
    } else {
        (red - green) / chroma + 4.0
    };
    ((sector * 60.0 / DEGREES_PER_BUCKET) as usize).min(HUE_BUCKETS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_of(width: u32, height: u32, pixel_at: impl Fn(u32, u32) -> [u8; 4]) -> DecodedImage {
        let rgba = (0..height)
            .flat_map(|row| (0..width).map(move |column| (row, column)))
            .flat_map(|(row, column)| pixel_at(column, row))
            .collect();
        DecodedImage {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn given_a_solid_red_cover_the_tint_is_red() {
        let cover = image_of(64, 64, |_, _| [220, 20, 30, 255]);
        assert_eq!(
            cover.dominant_tint(),
            Some(Rgb {
                red: 220,
                green: 20,
                blue: 30
            })
        );
    }

    #[test]
    fn given_a_black_cover_with_a_blue_patch_the_tint_is_the_patch() {
        let cover = image_of(200, 200, |column, row| match column < 60 && row < 60 {
            true => [30, 60, 230, 255],
            false => [4, 4, 4, 255],
        });
        let tint = cover.dominant_tint().unwrap();
        assert!(tint.blue > 200 && tint.red < 60, "{tint:?}");
    }

    #[test]
    fn given_two_hues_the_more_vivid_and_common_one_wins() {
        let cover = image_of(100, 100, |column, _| match column < 70 {
            true => [40, 200, 60, 255],
            false => [200, 40, 160, 255],
        });
        let tint = cover.dominant_tint().unwrap();
        assert!(tint.green > tint.red, "{tint:?}");
    }

    #[test]
    fn given_a_greyscale_cover_there_is_no_tint() {
        let cover = image_of(80, 80, |column, _| {
            let level = (column * 3) as u8;
            [level, level, level, 255]
        });
        assert_eq!(cover.dominant_tint(), None);
    }

    #[test]
    fn given_transparent_pixels_they_are_ignored() {
        let cover = image_of(32, 32, |_, _| [255, 0, 0, 0]);
        assert_eq!(cover.dominant_tint(), None);
    }

    #[test]
    fn given_an_empty_or_truncated_buffer_there_is_no_tint() {
        let empty = DecodedImage {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        let truncated = DecodedImage {
            width: 4,
            height: 4,
            rgba: vec![255; 10],
        };
        assert_eq!(empty.dominant_tint(), None);
        assert_eq!(truncated.dominant_tint(), None);
    }

    #[test]
    fn given_the_same_cover_the_tint_is_the_same() {
        let cover = image_of(300, 120, |column, row| {
            [(column % 256) as u8, (row * 2 % 256) as u8, 90, 255]
        });
        assert_eq!(cover.dominant_tint(), cover.dominant_tint());
        assert!(cover.dominant_tint().is_some());
    }
}
