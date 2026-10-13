use feishin_core::model::{ItemKind, Paging, SongListQuery, SongSort};
use slint::ComponentHandle;

use super::super::artists;
use super::super::{AppWindow, ArtistDetailState, Context, PlayerState, on_ui, show_status};
use super::current::{is_current, update_current};
use crate::controller::Command;

pub fn artist_songs_query(artist_id: &str) -> SongListQuery {
    SongListQuery {
        sort: SongSort::Album,
        artist_ids: vec![artist_id.to_owned()],
        ..SongListQuery::default()
    }
}

fn first_song_sort(shuffle: bool, discography_sort: SongSort) -> SongSort {
    match shuffle {
        true => SongSort::Random,
        false => discography_sort,
    }
}

pub fn play_artist(window: &AppWindow, ctx: &Context, artist_id: &str, shuffle: bool) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    if shuffle && !window.global::<PlayerState>().get_shuffle() {
        ctx.send(Command::ToggleShuffle);
    }
    let query = artist_songs_query(artist_id);
    let first_song_query = SongListQuery {
        sort: first_song_sort(shuffle, query.sort),
        ..query.clone()
    };
    let commands = ctx.clone();
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let paging = Paging {
            start_index: 0,
            limit: 1,
        };
        match server.song_list(&first_song_query, paging).await {
            Ok(page) => match page.items.into_iter().next() {
                Some(song) => commands.send(Command::PlayFromList {
                    query,
                    song_id: song.id,
                }),
                None => on_ui(&weak_window, |window| {
                    show_status(window, "This artist has no songs");
                }),
            },
            Err(error) => on_ui(&weak_window, move |window| {
                show_status(window, &format!("Could not play the artist: {error}"));
            }),
        }
    });
}

fn flip_loaded_favorite() -> Option<(u64, String, bool)> {
    update_current(|current| {
        if !current.loaded {
            return None;
        }
        current.favorite = !current.favorite;
        Some((current.generation, current.id.clone(), current.favorite))
    })
}

fn revert_favorite(window: &AppWindow, generation: u64, artist_id: &str, favorite: bool) {
    artists::set_favorite(window, artist_id, !favorite);
    if is_current(generation) {
        update_current(|current| current.favorite = !favorite);
        window.global::<ArtistDetailState>().set_favorite(!favorite);
    }
}

pub fn toggle_favorite(window: &AppWindow, ctx: &Context) {
    let Some((generation, artist_id, favorite)) = flip_loaded_favorite() else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    window.global::<ArtistDetailState>().set_favorite(favorite);
    artists::set_favorite(window, &artist_id, favorite);
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let artist_ids = [artist_id.clone()];
        let result = server
            .set_favorite(ItemKind::Artist, &artist_ids, favorite)
            .await;
        if let Err(error) = result {
            on_ui(&weak_window, move |window| {
                revert_favorite(window, generation, &artist_id, favorite);
                show_status(window, &format!("Could not update the favorite: {error}"));
            });
        }
    });
}

pub fn play_album(window: &AppWindow, ctx: &Context, album_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let commands = ctx.clone();
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        match server.album(&album_id).await {
            Ok(album) => commands.send(Command::PlaySongs {
                songs: album.songs,
                song_id: None,
            }),
            Err(error) => on_ui(&weak_window, move |window| {
                show_status(window, &format!("Could not play the album: {error}"));
            }),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_all_queries_every_song_of_the_artist() {
        let query = artist_songs_query("artist-7");
        assert_eq!(query.artist_ids, ["artist-7"]);
        assert_eq!(query.sort, SongSort::Album);
        assert!(query.search.is_none() && query.album_ids.is_empty());
    }
}
