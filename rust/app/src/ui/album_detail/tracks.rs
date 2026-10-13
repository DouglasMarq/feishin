use std::cell::RefCell;
use std::collections::BTreeSet;

use feishin_core::model::Song;
use slint::{ComponentHandle, Model, ModelTracker, SharedString, VecModel};

use super::super::{AlbumDetailState, AlbumDetailTrack, AppWindow, format_duration};
use crate::controller::artist_names;

#[derive(Default)]
pub struct AlbumTracks {
    pub rows: VecModel<AlbumDetailTrack>,
    pub songs: RefCell<Vec<Song>>,
}

impl Model for AlbumTracks {
    type Data = AlbumDetailTrack;

    fn row_count(&self) -> usize {
        self.rows.row_count()
    }

    fn row_data(&self, row: usize) -> Option<AlbumDetailTrack> {
        self.rows.row_data(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.rows.model_tracker()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn with_tracks<Output>(
    window: &AppWindow,
    action: impl FnOnce(&AlbumTracks) -> Output,
) -> Option<Output> {
    let tracks = window.global::<AlbumDetailState>().get_tracks();
    tracks.as_any().downcast_ref::<AlbumTracks>().map(action)
}

pub fn loaded_songs(window: &AppWindow) -> Vec<Song> {
    with_tracks(window, |tracks| tracks.songs.borrow().clone()).unwrap_or_default()
}

pub fn loaded_song(window: &AppWindow, song_id: &str) -> Option<Song> {
    with_tracks(window, |tracks| {
        tracks
            .songs
            .borrow()
            .iter()
            .find(|song| song.id == song_id)
            .cloned()
    })
    .flatten()
}

fn disc_of(song: &Song) -> u32 {
    song.disc_number.unwrap_or(1)
}

pub fn ordered(mut songs: Vec<Song>) -> Vec<Song> {
    songs.sort_by_key(|song| (disc_of(song), song.track_number.unwrap_or(u32::MAX)));
    songs
}

fn disc_header(disc: u32) -> AlbumDetailTrack {
    AlbumDetailTrack {
        disc_header: format!("Disc {disc}").into(),
        ..AlbumDetailTrack::default()
    }
}

fn track_row(song: &Song, position: usize) -> AlbumDetailTrack {
    AlbumDetailTrack {
        id: song.id.as_str().into(),
        number: song
            .track_number
            .unwrap_or(position as u32 + 1)
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
    }
}

pub fn track_rows(songs: &[Song]) -> Vec<AlbumDetailTrack> {
    let discs: BTreeSet<u32> = songs.iter().map(disc_of).collect();
    let has_several_discs = discs.len() > 1;
    let mut rows = Vec::with_capacity(songs.len() + discs.len());
    let mut previous_disc = None;
    for (position, song) in songs.iter().enumerate() {
        let disc = disc_of(song);
        if has_several_discs && previous_disc != Some(disc) {
            rows.push(disc_header(disc));
        }
        previous_disc = Some(disc);
        rows.push(track_row(song, position));
    }
    rows
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

    #[test]
    fn songs_are_ordered_and_grouped_by_disc() {
        let songs = ordered(vec![song("c", 2, 1), song("b", 1, 2), song("a", 1, 1)]);
        let ids: Vec<&str> = songs.iter().map(|song| song.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);

        let rows = track_rows(&songs);
        let shown: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| (row.disc_header.as_str(), row.id.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Disc 1", ""),
                ("", "a"),
                ("", "b"),
                ("Disc 2", ""),
                ("", "c")
            ]
        );
        assert_eq!(rows[1].number, "1");
        assert_eq!(rows[1].duration, "1:01");
    }

    #[test]
    fn single_disc_albums_have_no_headers() {
        let rows = track_rows(&[song("a", 1, 1), song("b", 1, 2)]);
        assert!(rows.iter().all(|row| row.disc_header.is_empty()));
        assert_eq!(rows.len(), 2);
    }
}
