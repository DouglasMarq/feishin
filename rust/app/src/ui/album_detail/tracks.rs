use std::cell::RefCell;

use feishin_core::model::Song;
use slint::{ComponentHandle, Model, ModelNotify, ModelTracker, SharedString};

use super::super::{AlbumDetailState, AlbumDetailTrack, AppWindow, format_duration};
use super::layout::{AlbumRow, PlacedRow, TrackLayout, disc_of};
use crate::controller::artist_names;

#[derive(Default)]
struct AlbumTrackList {
    songs: Vec<Song>,
    layout: TrackLayout,
}

#[derive(Default)]
pub struct AlbumTracks {
    list: RefCell<AlbumTrackList>,
    notify: ModelNotify,
}

impl Model for AlbumTracks {
    type Data = AlbumDetailTrack;

    fn row_count(&self) -> usize {
        self.list.borrow().layout.len()
    }

    fn row_data(&self, row: usize) -> Option<AlbumDetailTrack> {
        let list = self.list.borrow();
        let placed = list.layout.placed(row)?;
        match placed.row {
            AlbumRow::Disc(disc) => Some(disc_header(disc, placed)),
            AlbumRow::Track(position) => list
                .songs
                .get(position)
                .map(|song| track_row(song, position, placed)),
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AlbumTracks {
    pub fn replace(&self, songs: Vec<Song>) {
        let songs = ordered(songs);
        let layout = TrackLayout::new(&songs);
        *self.list.borrow_mut() = AlbumTrackList { songs, layout };
        self.notify.reset();
    }

    pub fn songs(&self) -> Vec<Song> {
        self.list.borrow().songs.clone()
    }

    pub fn song(&self, song_id: &str) -> Option<Song> {
        let list = self.list.borrow();
        list.songs.iter().find(|song| song.id == song_id).cloned()
    }

    pub fn height(&self) -> f32 {
        self.list.borrow().layout.height()
    }

    pub fn visible_slots(&self, viewport_height: f32) -> usize {
        self.list.borrow().layout.visible_slots(viewport_height)
    }

    pub fn first_visible_row(&self, viewport_top: f32, slots: usize) -> usize {
        self.list
            .borrow()
            .layout
            .first_visible_row(viewport_top, slots)
    }
}

pub fn with_tracks<Output>(
    window: &AppWindow,
    action: impl FnOnce(&AlbumTracks) -> Output,
) -> Option<Output> {
    let tracks = window.global::<AlbumDetailState>().get_tracks();
    tracks.as_any().downcast_ref::<AlbumTracks>().map(action)
}

pub fn show_tracks(window: &AppWindow, songs: Vec<Song>) {
    let height = with_tracks(window, |tracks| {
        tracks.replace(songs);
        tracks.height()
    });
    window
        .global::<AlbumDetailState>()
        .set_tracks_height(height.unwrap_or_default());
}

pub fn loaded_songs(window: &AppWindow) -> Vec<Song> {
    with_tracks(window, AlbumTracks::songs).unwrap_or_default()
}

pub fn loaded_song(window: &AppWindow, song_id: &str) -> Option<Song> {
    with_tracks(window, |tracks| tracks.song(song_id)).flatten()
}

fn ordered(mut songs: Vec<Song>) -> Vec<Song> {
    songs.sort_by_key(|song| (disc_of(song), song.track_number.unwrap_or(u32::MAX)));
    songs
}

fn disc_header(disc: u32, placed: PlacedRow) -> AlbumDetailTrack {
    AlbumDetailTrack {
        disc_header: format!("Disc {disc}").into(),
        top: placed.top,
        height: placed.height,
        ..AlbumDetailTrack::default()
    }
}

fn track_number_from_position(position: usize) -> u32 {
    u32::try_from(position + 1).unwrap_or(u32::MAX)
}

fn track_row(song: &Song, position: usize, placed: PlacedRow) -> AlbumDetailTrack {
    AlbumDetailTrack {
        id: song.id.as_str().into(),
        number: song
            .track_number
            .unwrap_or_else(|| track_number_from_position(position))
            .to_string()
            .into(),
        title: song.title.as_str().into(),
        artists: artist_names(song).into(),
        artist_id: song
            .artists
            .first()
            .map(|artist| artist.id.as_str())
            .unwrap_or_default()
            .into(),
        plays: song.play_count.to_string().into(),
        duration: format_duration(song.duration_ms).into(),
        disc_header: SharedString::default(),
        top: placed.top,
        height: placed.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    fn song(id: &str, disc: u32, track: u32) -> Song {
        Song {
            disc_number: Some(disc),
            track_number: Some(track),
            ..test_song(id, 61_000)
        }
    }

    fn rows_of(songs: Vec<Song>) -> Vec<AlbumDetailTrack> {
        let tracks = AlbumTracks::default();
        tracks.replace(songs);
        tracks.iter().collect()
    }

    #[test]
    fn songs_are_ordered_and_grouped_by_disc() {
        let tracks = AlbumTracks::default();
        tracks.replace(vec![song("c", 2, 1), song("b", 1, 2), song("a", 1, 1)]);
        let ids: Vec<String> = tracks.songs().into_iter().map(|song| song.id).collect();
        assert_eq!(ids, ["a", "b", "c"]);

        let rows: Vec<AlbumDetailTrack> = tracks.iter().collect();
        let shown: Vec<(&str, &str, f32)> = rows
            .iter()
            .map(|row| (row.disc_header.as_str(), row.id.as_str(), row.top))
            .collect();
        assert_eq!(
            shown,
            [
                ("Disc 1", "", 0.0),
                ("", "a", 44.0),
                ("", "b", 80.0),
                ("Disc 2", "", 116.0),
                ("", "c", 160.0)
            ]
        );
        assert_eq!(rows[1].number, "1");
        assert_eq!(rows[1].duration, "1:01");
        assert_eq!(tracks.height(), 196.0);
    }

    #[test]
    fn untracked_songs_are_numbered_by_position() {
        let untracked = Song {
            track_number: None,
            ..song("c", 1, 1)
        };
        let rows = rows_of(vec![song("a", 1, 1), song("b", 1, 2), untracked]);
        assert_eq!(rows[2].number, "3");
    }

    #[test]
    fn single_disc_albums_have_no_headers() {
        let rows = rows_of(vec![song("a", 1, 1), song("b", 1, 2)]);
        assert!(rows.iter().all(|row| row.disc_header.is_empty()));
        assert_eq!(rows.len(), 2);
    }
}
