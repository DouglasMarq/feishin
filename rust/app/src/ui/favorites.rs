use feishin_core::model::{AlbumListQuery, ArtistListQuery, SongListQuery};
use slint::ComponentHandle;

use super::genres::{
    Binding, CardSource, card_grid, play_track, toggle_card, toggle_track, track_table, with_grid,
    with_table,
};
use super::{AppWindow, CardRow, Context, FavoritesState, TrackItem};

pub fn favorite_song_query() -> SongListQuery {
    SongListQuery {
        favorite: Some(true),
        ..SongListQuery::default()
    }
}

pub fn favorite_album_query() -> AlbumListQuery {
    AlbumListQuery {
        favorite: Some(true),
        ..AlbumListQuery::default()
    }
}

pub fn favorite_artist_query() -> ArtistListQuery {
    ArtistListQuery {
        favorite: Some(true),
        ..ArtistListQuery::default()
    }
}

const TRACKS: Binding<TrackItem> = Binding {
    rows: |window| window.global::<FavoritesState>().get_tracks(),
    loading: |window, loading| {
        window
            .global::<FavoritesState>()
            .set_tracks_loading(loading)
    },
};

const ALBUMS: Binding<CardRow> = Binding {
    rows: |window| window.global::<FavoritesState>().get_albums(),
    loading: |window, loading| {
        window
            .global::<FavoritesState>()
            .set_albums_loading(loading)
    },
};

const ARTISTS: Binding<CardRow> = Binding {
    rows: |window| window.global::<FavoritesState>().get_artists(),
    loading: |window, loading| {
        window
            .global::<FavoritesState>()
            .set_artists_loading(loading)
    },
};

fn columns(window: &AppWindow, binding: Binding<CardRow>, columns: i32) {
    with_grid(window, binding, |grid| {
        grid.set_columns(usize::try_from(columns).unwrap_or(1))
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<FavoritesState>();
    state.set_tracks(track_table(window, ctx, true, TRACKS));
    state.set_albums(card_grid(window, ctx, ALBUMS));
    state.set_artists(card_grid(window, ctx, ARTISTS));

    let weak = window.as_weak();
    state.on_load(move |_| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<FavoritesState>();
        state.set_tracks_loading(true);
        state.set_albums_loading(true);
        state.set_artists_loading(true);
        with_table(&window, TRACKS, |table| table.reset(favorite_song_query()));
        with_grid(&window, ALBUMS, |grid| {
            grid.reset(Some(CardSource::Albums(favorite_album_query())))
        });
        with_grid(&window, ARTISTS, |grid| {
            grid.reset(Some(CardSource::Artists(favorite_artist_query())))
        });
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play_track(move |index| {
        if let Some(window) = weak.upgrade() {
            play_track(&window, &play_ctx, TRACKS, index);
        }
    });

    let weak = window.as_weak();
    let toggle_ctx = ctx.clone();
    state.on_toggle_track(move |index| {
        if let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) {
            toggle_track(&window, &toggle_ctx, TRACKS, index);
        }
    });

    let weak = window.as_weak();
    let toggle_ctx = ctx.clone();
    state.on_toggle_album(move |id| {
        if let Some(window) = weak.upgrade() {
            toggle_card(&window, &toggle_ctx, ALBUMS, &id);
        }
    });

    let weak = window.as_weak();
    let toggle_ctx = ctx.clone();
    state.on_toggle_artist(move |id| {
        if let Some(window) = weak.upgrade() {
            toggle_card(&window, &toggle_ctx, ARTISTS, &id);
        }
    });

    let weak = window.as_weak();
    state.on_album_columns(move |n| {
        if let Some(window) = weak.upgrade() {
            columns(&window, ALBUMS, n);
        }
    });

    let weak = window.as_weak();
    state.on_artist_columns(move |n| {
        if let Some(window) = weak.upgrade() {
            columns(&window, ARTISTS, n);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::FakeServer;
    use feishin_api::MusicServer;
    use feishin_core::model::Paging;

    const ALL: Paging = Paging {
        start_index: 0,
        limit: usize::MAX,
    };

    #[test]
    fn queries_ask_for_favorites_only() {
        assert_eq!(favorite_song_query().favorite, Some(true));
        assert_eq!(favorite_album_query().favorite, Some(true));
        assert_eq!(favorite_artist_query().favorite, Some(true));
    }

    #[tokio::test]
    async fn demo_server_lists_favorites() {
        let server = FakeServer::new(1200);
        let songs = server
            .song_list(&favorite_song_query(), ALL)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!songs.is_empty() && songs.iter().all(|song| song.favorite));
        let albums = server
            .album_list(&favorite_album_query(), ALL)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!albums.is_empty() && albums.len() < 100);
        assert!(albums.iter().all(|album| album.favorite));
        let artists = server
            .artist_list(&favorite_artist_query(), ALL)
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert!(!artists.is_empty() && artists.iter().all(|artist| artist.favorite));
    }
}
