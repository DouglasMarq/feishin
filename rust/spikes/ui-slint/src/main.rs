use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use slint::{Image, Model, ModelRc, ModelTracker, Rgba8Pixel, SharedPixelBuffer, VecModel};

slint::include_modules!();

const TRACK_COUNT: usize = 20_000;
const ALBUM_COUNT: usize = 3_000;
const CARDS_PER_ROW: usize = 6;
const COVER_SIZE: u32 = 256;
const COVER_CACHE_CAPACITY: usize = 120;

struct CoverCache {
    images: HashMap<usize, Image>,
    order: VecDeque<usize>,
}

impl CoverCache {
    fn get(&mut self, album: usize) -> Image {
        if let Some(image) = self.images.get(&album) {
            return image.clone();
        }

        let image = render_cover(album);
        self.images.insert(album, image.clone());
        self.order.push_back(album);

        while self.order.len() > COVER_CACHE_CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }

        image
    }
}

fn render_cover(album: usize) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(COVER_SIZE, COVER_SIZE);
    let seed = album as u32;
    for (i, pixel) in buffer.make_mut_slice().iter_mut().enumerate() {
        let x = i as u32 % COVER_SIZE;
        let y = i as u32 / COVER_SIZE;
        *pixel = Rgba8Pixel {
            r: (x.wrapping_mul(seed % 7 + 1) % 256) as u8,
            g: (y.wrapping_mul(seed % 5 + 1) % 256) as u8,
            b: ((x + y + seed * 37) % 256) as u8,
            a: 255,
        };
    }
    Image::from_rgba8(buffer)
}

struct AlbumRowsModel {
    covers: RefCell<CoverCache>,
}

impl Model for AlbumRowsModel {
    type Data = AlbumRow;

    fn row_count(&self) -> usize {
        ALBUM_COUNT.div_ceil(CARDS_PER_ROW)
    }

    fn row_data(&self, row: usize) -> Option<AlbumRow> {
        if row >= self.row_count() {
            return None;
        }

        let start = row * CARDS_PER_ROW;
        let end = (start + CARDS_PER_ROW).min(ALBUM_COUNT);
        let mut covers = self.covers.borrow_mut();
        let cards: Vec<AlbumCard> = (start..end)
            .map(|album| AlbumCard {
                name: format!("Album {album}").into(),
                artist: format!("Artist {}", album % 400).into(),
                cover: covers.get(album),
            })
            .collect();

        Some(AlbumRow {
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let tracks: Vec<Track> = (0..TRACK_COUNT)
        .map(|i| Track {
            title: format!("Track title number {i}").into(),
            artist: format!("Artist {}", i % 400).into(),
            album: format!("Album {}", i % ALBUM_COUNT).into(),
            duration: format!("{}:{:02}", 2 + i % 5, i % 60).into(),
        })
        .collect();

    let window = AppWindow::new()?;
    window.set_tracks(Rc::new(VecModel::from(tracks)).into());
    window.set_album_rows(ModelRc::new(AlbumRowsModel {
        covers: RefCell::new(CoverCache {
            images: HashMap::new(),
            order: VecDeque::new(),
        }),
    }));

    let autoscroll = std::env::var("SPIKE_AUTOSCROLL").unwrap_or_default();
    window.set_initial_tab(if autoscroll == "albums" { 1 } else { 0 });
    window.set_autoscroll(autoscroll.into());

    window.run()
}
