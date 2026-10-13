use std::cell::RefCell;
use std::collections::BTreeSet;

use feishin_core::model::{AlbumDetail, ItemKind, Song};
use slint::{ComponentHandle, Model, ModelRc, ModelTracker, SharedString, VecModel};

use super::{
    AlbumDetailArtist, AlbumDetailState, AlbumDetailTrack, AppWindow, Context, PlayerState,
    format_duration, on_ui, show_status,
};
use crate::controller::{Command, artist_names};

const COVER_SIZE: u32 = 440;

/// Track rows plus the songs behind them, so playback commands can reuse what was loaded.
struct AlbumTracks {
    rows: VecModel<AlbumDetailTrack>,
    songs: RefCell<Vec<Song>>,
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

fn with_tracks<R>(window: &AppWindow, f: impl FnOnce(&AlbumTracks) -> R) -> Option<R> {
    let tracks = window.global::<AlbumDetailState>().get_tracks();
    tracks.as_any().downcast_ref::<AlbumTracks>().map(f)
}

fn songs(window: &AppWindow) -> Vec<Song> {
    with_tracks(window, |tracks| tracks.songs.borrow().clone()).unwrap_or_default()
}

/// Songs in disc and track order.
pub fn ordered(mut songs: Vec<Song>) -> Vec<Song> {
    songs.sort_by_key(|song| {
        (
            song.disc_number.unwrap_or(1),
            song.track_number.unwrap_or(u32::MAX),
        )
    });
    songs
}

/// Rows for `songs` (already ordered), with a header before each disc when there is more than one.
pub fn track_rows(songs: &[Song]) -> Vec<AlbumDetailTrack> {
    let discs: BTreeSet<u32> = songs
        .iter()
        .map(|song| song.disc_number.unwrap_or(1))
        .collect();
    let mut rows = Vec::with_capacity(songs.len() + discs.len());
    let mut disc = None;
    for (i, song) in songs.iter().enumerate() {
        let number = song.disc_number.unwrap_or(1);
        if discs.len() > 1 && disc != Some(number) {
            rows.push(AlbumDetailTrack {
                disc_header: format!("Disc {number}").into(),
                ..AlbumDetailTrack::default()
            });
        }
        disc = Some(number);
        rows.push(AlbumDetailTrack {
            id: song.id.as_str().into(),
            number: song.track_number.unwrap_or(i as u32 + 1).to_string().into(),
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
        });
    }
    rows
}

/// "1999 • Rock, Jazz • 12 songs • 48:10"
pub fn album_info(detail: &AlbumDetail) -> String {
    let album = &detail.album;
    let count = if detail.songs.is_empty() {
        album.song_count as usize
    } else {
        detail.songs.len()
    };
    let duration = if detail.songs.is_empty() {
        album.duration_ms
    } else {
        detail.songs.iter().map(|song| song.duration_ms).sum()
    };
    let mut parts = Vec::new();
    if let Some(year) = album.year {
        parts.push(year.to_string());
    }
    if !album.genres.is_empty() {
        parts.push(album.genres.join(", "));
    }
    parts.push(format!(
        "{count} {}",
        if count == 1 { "song" } else { "songs" }
    ));
    parts.push(format_duration(duration));
    parts.join(" • ")
}

fn show(window: &AppWindow, detail: AlbumDetail) {
    let state = window.global::<AlbumDetailState>();
    let album = &detail.album;
    let artists = if album.album_artists.is_empty() {
        &album.artists
    } else {
        &album.album_artists
    };
    let artists: Vec<AlbumDetailArtist> = artists
        .iter()
        .map(|artist| AlbumDetailArtist {
            id: artist.id.as_str().into(),
            name: artist.name.as_str().into(),
        })
        .collect();
    state.set_name(album.name.as_str().into());
    state.set_artists(ModelRc::new(VecModel::from(artists)));
    state.set_info(album_info(&detail).into());
    state.set_favorite(album.favorite);
    state.set_rating(i32::from(album.user_rating.unwrap_or(0)));
    let songs = ordered(detail.songs);
    let rows = track_rows(&songs);
    with_tracks(window, |tracks| {
        tracks.rows.set_vec(rows);
        *tracks.songs.borrow_mut() = songs;
    });
    state.set_loading(false);
}

fn clear(window: &AppWindow, id: &SharedString) {
    let state = window.global::<AlbumDetailState>();
    state.set_album_id(id.clone());
    state.set_loading(true);
    state.set_error(SharedString::default());
    state.set_name(SharedString::default());
    state.set_artists(ModelRc::default());
    state.set_info(SharedString::default());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_favorite(false);
    state.set_rating(0);
    with_tracks(window, |tracks| {
        tracks.rows.clear();
        tracks.songs.borrow_mut().clear();
    });
}

fn load(window: &AppWindow, ctx: &Context, id: SharedString) {
    clear(window, &id);
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        let result = server.album(&id).await;
        let cover_id = result
            .as_ref()
            .ok()
            .and_then(|detail| detail.album.cover_art_id.clone());
        let shown_id = id.clone();
        on_ui(&weak, move |window| {
            if window.global::<AlbumDetailState>().get_album_id() != shown_id {
                return;
            }
            match result {
                Ok(detail) => show(window, detail),
                Err(e) => {
                    let state = window.global::<AlbumDetailState>();
                    state.set_loading(false);
                    state.set_error("Album not found".into());
                    show_status(window, &format!("Could not load the album: {e}"));
                }
            }
        });
        let Some(cover_id) = cover_id else {
            return;
        };
        match covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await {
            Ok(cover) => on_ui(&weak, move |window| {
                if window.global::<AlbumDetailState>().get_album_id() == id
                    && let Some(image) = super::player::to_image(&cover)
                {
                    let state = window.global::<AlbumDetailState>();
                    state.set_cover(image);
                    state.set_has_cover(true);
                }
            }),
            Err(e) => tracing::debug!("cover {cover_id}: {e:#}"),
        }
    });
}

fn set_favorite(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumDetailState>();
    let id = state.get_album_id();
    let Some(server) = ctx.session.get().filter(|_| !id.is_empty()) else {
        return;
    };
    let favorite = !state.get_favorite();
    state.set_favorite(favorite);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server
            .set_favorite(ItemKind::Album, &[id.to_string()], favorite)
            .await;
        if let Err(e) = result {
            on_ui(&weak, move |window| {
                let state = window.global::<AlbumDetailState>();
                if state.get_album_id() == id {
                    state.set_favorite(!favorite);
                }
                show_status(window, &format!("Could not update favorite: {e}"));
            });
        }
    });
}

fn set_rating(window: &AppWindow, ctx: &Context, rating: i32) {
    let state = window.global::<AlbumDetailState>();
    let id = state.get_album_id();
    let Some(server) = ctx.session.get().filter(|_| !id.is_empty()) else {
        return;
    };
    let previous = state.get_rating();
    let rating = rating.clamp(0, 5);
    state.set_rating(rating);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        if let Err(e) = server.set_rating(ItemKind::Album, &id, rating as u8).await {
            on_ui(&weak, move |window| {
                let state = window.global::<AlbumDetailState>();
                if state.get_album_id() == id {
                    state.set_rating(previous);
                }
                show_status(window, &format!("Could not update rating: {e}"));
            });
        }
    });
}

fn song(window: &AppWindow, id: &str) -> Option<Song> {
    with_tracks(window, |tracks| {
        tracks.songs.borrow().iter().find(|s| s.id == id).cloned()
    })
    .flatten()
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumDetailState>();
    state.set_tracks(ModelRc::new(AlbumTracks {
        rows: VecModel::default(),
        songs: RefCell::default(),
    }));

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    state.on_load(move |id| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx, id);
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play(move || {
        if let Some(window) = weak.upgrade() {
            let songs = songs(&window);
            if !songs.is_empty() {
                play_ctx.send(Command::PlaySongs {
                    songs,
                    song_id: None,
                });
            }
        }
    });

    let weak = window.as_weak();
    let shuffle_ctx = ctx.clone();
    state.on_shuffle(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let songs = songs(&window);
        if songs.is_empty() {
            return;
        }
        if !window.global::<PlayerState>().get_shuffle() {
            shuffle_ctx.send(Command::ToggleShuffle);
        }
        shuffle_ctx.send(Command::PlaySongs {
            songs,
            song_id: None,
        });
    });

    let weak = window.as_weak();
    let track_ctx = ctx.clone();
    state.on_play_track(move |id| {
        if let Some(window) = weak.upgrade() {
            let songs = songs(&window);
            if songs.iter().any(|song| song.id == id.as_str()) {
                track_ctx.send(Command::PlaySongs {
                    songs,
                    song_id: Some(id.into()),
                });
            }
        }
    });

    let weak = window.as_weak();
    let next_ctx = ctx.clone();
    state.on_play_next(move |id| {
        if let Some(song) = weak.upgrade().and_then(|window| song(&window, &id)) {
            next_ctx.send(Command::AddNext(vec![song]));
        }
    });

    let weak = window.as_weak();
    let queue_ctx = ctx.clone();
    state.on_add_to_queue(move |id| {
        if let Some(song) = weak.upgrade().and_then(|window| song(&window, &id)) {
            queue_ctx.send(Command::AddLast(vec![song]));
        }
    });

    let weak = window.as_weak();
    let favorite_ctx = ctx.clone();
    state.on_toggle_favorite(move || {
        if let Some(window) = weak.upgrade() {
            set_favorite(&window, &favorite_ctx);
        }
    });

    let weak = window.as_weak();
    let rating_ctx = ctx.clone();
    state.on_set_rating(move |rating| {
        if let Some(window) = weak.upgrade() {
            set_rating(&window, &rating_ctx, rating);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{FakeServer, test_song};
    use feishin_api::MusicServer;

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
        let ids: Vec<&str> = songs.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);

        let rows = track_rows(&songs);
        let shown: Vec<(&str, &str)> = rows
            .iter()
            .map(|r| (r.disc_header.as_str(), r.id.as_str()))
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
        assert!(rows.iter().all(|r| r.disc_header.is_empty()));
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn info_summarizes_the_album() {
        let server = FakeServer::new(24);
        let detail = server.album("album-0").await.expect("album");
        let info = album_info(&detail);
        let year = detail.album.year.unwrap_or_default().to_string();
        assert!(info.starts_with(&year));
        assert!(info.contains("12 songs"));
        assert!(info.ends_with(&format_duration(detail.album.duration_ms)));
    }
}
