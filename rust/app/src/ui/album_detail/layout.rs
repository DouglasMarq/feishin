use feishin_core::model::Song;

const TRACK_ROW_HEIGHT: f32 = 36.0;
const DISC_ROW_HEIGHT: f32 = 44.0;
const OVERSCAN_SLOTS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlbumRow {
    Disc(u32),
    Track(usize),
}

impl AlbumRow {
    fn height(self) -> f32 {
        match self {
            Self::Disc(_) => DISC_ROW_HEIGHT,
            Self::Track(_) => TRACK_ROW_HEIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedRow {
    pub row: AlbumRow,
    pub top: f32,
    pub height: f32,
}

#[derive(Default)]
pub struct TrackLayout {
    rows: Vec<AlbumRow>,
    tops: Vec<f32>,
    height: f32,
}

pub fn disc_of(song: &Song) -> u32 {
    song.disc_number.unwrap_or(1)
}

fn has_several_discs(songs: &[Song]) -> bool {
    let mut discs = songs.iter().map(disc_of);
    let first_disc = discs.next();
    discs.any(|disc| Some(disc) != first_disc)
}

fn album_rows(songs: &[Song]) -> Vec<AlbumRow> {
    let has_several_discs = has_several_discs(songs);
    let mut rows = Vec::with_capacity(songs.len());
    let mut previous_disc = None;
    for (position, song) in songs.iter().enumerate() {
        let disc = disc_of(song);
        if has_several_discs && previous_disc != Some(disc) {
            rows.push(AlbumRow::Disc(disc));
        }
        previous_disc = Some(disc);
        rows.push(AlbumRow::Track(position));
    }
    rows
}

impl TrackLayout {
    pub fn new(songs: &[Song]) -> Self {
        let rows = album_rows(songs);
        let mut tops = Vec::with_capacity(rows.len());
        let height = rows.iter().fold(0.0, |top, row| {
            tops.push(top);
            top + row.height()
        });
        Self { rows, tops, height }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn placed(&self, index: usize) -> Option<PlacedRow> {
        let row = *self.rows.get(index)?;
        let top = *self.tops.get(index)?;
        Some(PlacedRow {
            row,
            top,
            height: row.height(),
        })
    }

    pub fn visible_slots(&self, viewport_height: f32) -> usize {
        let rows_in_view = (viewport_height.max(0.0) / TRACK_ROW_HEIGHT).ceil() as usize;
        (rows_in_view + OVERSCAN_SLOTS).min(self.len())
    }

    pub fn first_visible_row(&self, viewport_top: f32, slots: usize) -> usize {
        let row_at_top = self
            .tops
            .partition_point(|top| *top <= viewport_top)
            .saturating_sub(1);
        row_at_top.min(self.len().saturating_sub(slots))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    fn songs_on_discs(discs: &[u32]) -> Vec<Song> {
        discs
            .iter()
            .enumerate()
            .map(|(position, disc)| Song {
                disc_number: Some(*disc),
                ..test_song(&format!("song-{position}"), 1_000)
            })
            .collect()
    }

    #[test]
    fn rows_are_found_by_their_offset() {
        let layout = TrackLayout::new(&songs_on_discs(&[1, 1, 2, 2, 2]));
        assert_eq!(layout.len(), 7);
        assert_eq!(
            layout.height(),
            2.0 * DISC_ROW_HEIGHT + 5.0 * TRACK_ROW_HEIGHT
        );
        assert_eq!(layout.first_visible_row(-120.0, 0), 0);
        assert_eq!(layout.first_visible_row(43.0, 0), 0);
        assert_eq!(layout.first_visible_row(44.0, 0), 1);
        assert_eq!(layout.first_visible_row(120.0, 0), 3);
        assert_eq!(layout.first_visible_row(10_000.0, 0), 6);
        assert_eq!(layout.first_visible_row(10_000.0, 3), 4);
        assert_eq!(
            layout.placed(3),
            Some(PlacedRow {
                row: AlbumRow::Disc(2),
                top: 116.0,
                height: DISC_ROW_HEIGHT
            })
        );
        assert_eq!(layout.placed(7), None);
    }

    #[test]
    fn slots_cover_the_viewport_without_exceeding_the_rows() {
        let box_set = TrackLayout::new(&songs_on_discs(&[1; 2_000]));
        assert_eq!(box_set.len(), 2_000);
        assert_eq!(box_set.visible_slots(720.0), 22);
        assert_eq!(box_set.visible_slots(-5.0), 2);
        let single = TrackLayout::new(&songs_on_discs(&[1]));
        assert_eq!(single.visible_slots(720.0), 1);
        assert_eq!(TrackLayout::default().visible_slots(720.0), 0);
    }
}
