use feishin_core::model::{AlbumListQuery, ArtistListQuery, SongListQuery};
use slint::ComponentHandle;

use super::genres::{
    Binding, CardSource, card_grid, play_track, set_grid_columns, toggle_card, toggle_track,
    track_table, with_grid, with_table,
};
use super::{AppWindow, CardRow, Context, FavoritesState, TrackItem};

pub fn favorite_song_query() -> SongListQuery {
    SongListQuery {
        is_favorite: Some(true),
        ..SongListQuery::default()
    }
}

pub fn favorite_album_query() -> AlbumListQuery {
    AlbumListQuery {
        is_favorite: Some(true),
        ..AlbumListQuery::default()
    }
}

pub fn favorite_artist_query() -> ArtistListQuery {
    ArtistListQuery {
        is_favorite: Some(true),
        ..ArtistListQuery::default()
    }
}

const TRACKS: Binding<TrackItem> = Binding {
    rows: |window| window.global::<FavoritesState>().get_tracks(),
    loading: |window, is_loading| {
        window
            .global::<FavoritesState>()
            .set_is_tracks_loading(is_loading)
    },
};

const ALBUMS: Binding<CardRow> = Binding {
    rows: |window| window.global::<FavoritesState>().get_albums(),
    loading: |window, is_loading| {
        window
            .global::<FavoritesState>()
            .set_is_albums_loading(is_loading)
    },
};

const ARTISTS: Binding<CardRow> = Binding {
    rows: |window| window.global::<FavoritesState>().get_artists(),
    loading: |window, is_loading| {
        window
            .global::<FavoritesState>()
            .set_is_artists_loading(is_loading)
    },
};

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<FavoritesState>();
    state.set_tracks(track_table(window, context, true, TRACKS));
    state.set_albums(card_grid(window, context, ALBUMS));
    state.set_artists(card_grid(window, context, ARTISTS));
    install_load(window);
    install_play_track(window, context);
    install_toggles(window, context);
    install_columns(window);
}

fn install_load(window: &AppWindow) {
    let weak = window.as_weak();
    window.global::<FavoritesState>().on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            load_favorites(&window);
        }
    });
}

fn load_favorites(window: &AppWindow) {
    let state = window.global::<FavoritesState>();
    state.set_is_tracks_loading(true);
    state.set_is_albums_loading(true);
    state.set_is_artists_loading(true);
    with_table(window, TRACKS, |table| table.reset(favorite_song_query()));
    with_grid(window, ALBUMS, |grid| {
        grid.reset(Some(CardSource::Albums(favorite_album_query())));
    });
    with_grid(window, ARTISTS, |grid| {
        grid.reset(Some(CardSource::Artists(favorite_artist_query())));
    });
}

fn install_play_track(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let play_context = context.clone();
    window
        .global::<FavoritesState>()
        .on_play_track(move |index| {
            if let Some(window) = weak.upgrade() {
                play_track(&window, &play_context, TRACKS, index);
            }
        });
}

fn install_toggles(window: &AppWindow, context: &Context) {
    let state = window.global::<FavoritesState>();

    let weak = window.as_weak();
    let toggle_context = context.clone();
    state.on_toggle_track(move |index| {
        if let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) {
            toggle_track(&window, &toggle_context, TRACKS, index);
        }
    });

    let weak = window.as_weak();
    let toggle_context = context.clone();
    state.on_toggle_album(move |id| {
        if let Some(window) = weak.upgrade() {
            toggle_card(&window, &toggle_context, ALBUMS, &id);
        }
    });

    let weak = window.as_weak();
    let toggle_context = context.clone();
    state.on_toggle_artist(move |id| {
        if let Some(window) = weak.upgrade() {
            toggle_card(&window, &toggle_context, ARTISTS, &id);
        }
    });
}

fn install_columns(window: &AppWindow) {
    let state = window.global::<FavoritesState>();

    let weak = window.as_weak();
    state.on_album_columns(move |columns| {
        if let Some(window) = weak.upgrade() {
            set_grid_columns(&window, ALBUMS, columns);
        }
    });

    let weak = window.as_weak();
    state.on_artist_columns(move |columns| {
        if let Some(window) = weak.upgrade() {
            set_grid_columns(&window, ARTISTS, columns);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::FakeServer;
    use feishin_api::MusicServer;
    use feishin_core::model::Paging;

    const EVERYTHING: Paging = Paging {
        start_index: 0,
        limit: usize::MAX,
    };

    #[test]
    fn queries_ask_for_favorites_only() {
        assert_eq!(favorite_song_query().is_favorite, Some(true));
        assert_eq!(favorite_album_query().is_favorite, Some(true));
        assert_eq!(favorite_artist_query().is_favorite, Some(true));
    }

    #[tokio::test]
    async fn demo_server_lists_favorites() {
        let server = FakeServer::new(1200);
        let songs = server
            .song_list(&favorite_song_query(), EVERYTHING)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!songs.is_empty() && songs.iter().all(|song| song.is_favorite));
        let albums = server
            .album_list(&favorite_album_query(), EVERYTHING)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!albums.is_empty() && albums.len() < 100);
        assert!(albums.iter().all(|album| album.is_favorite));
        let artists = server
            .artist_list(&favorite_artist_query(), EVERYTHING)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!artists.is_empty() && artists.iter().all(|artist| artist.is_favorite));
    }
}
