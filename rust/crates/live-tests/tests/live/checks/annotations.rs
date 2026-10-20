use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{ItemKind, SongListQuery};

use crate::support::{Backend, album_named, artist_named, everything, song_titled, titles};

pub async fn favorites_songs(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, "Late Set").await;
    let ids = [song.id.clone()];
    server
        .set_favorite(ItemKind::Song, &ids, true)
        .await
        .expect("favorite");
    assert!(server.song(&song.id).await.expect("song").is_favorite);
    let favorites = SongListQuery {
        is_favorite: Some(true),
        ..SongListQuery::default()
    };
    let listed = server
        .song_list(&favorites, everything())
        .await
        .expect("favorite songs");
    assert_eq!(titles(&listed.items), ["Late Set"]);
    server
        .set_favorite(ItemKind::Song, &ids, false)
        .await
        .expect("unfavorite");
    assert!(!server.song(&song.id).await.expect("song").is_favorite);
}

async fn is_favorite(server: &dyn MusicServer, kind: ItemKind, id: &str) -> bool {
    match kind {
        ItemKind::Song => server.song(id).await.expect("song").is_favorite,
        ItemKind::Album => server.album(id).await.expect("album").album.is_favorite,
        ItemKind::Artist => server.artist(id).await.expect("artist").artist.is_favorite,
        ItemKind::Playlist => panic!("playlists have no favorite flag"),
    }
}

async fn assert_favorite_toggles(server: &dyn MusicServer, kind: ItemKind, id: &str) {
    let ids = [id.to_owned()];
    server
        .set_favorite(kind, &ids, true)
        .await
        .expect("favorite");
    assert!(is_favorite(server, kind, id).await, "{kind:?} favorite");
    server
        .set_favorite(kind, &ids, false)
        .await
        .expect("unfavorite");
    assert!(!is_favorite(server, kind, id).await, "{kind:?} unfavorite");
}

pub async fn favorites_albums_and_artists(backend: Backend) {
    let server = backend.connect().await;
    let album = album_named(&*server, "Second Wave").await;
    assert_favorite_toggles(&*server, ItemKind::Album, &album.id).await;
    let artist = artist_named(&*server, "Charlie Square").await;
    assert_favorite_toggles(&*server, ItemKind::Artist, &artist.id).await;
}

pub async fn rates_songs_and_albums(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, "Blue Note").await;
    let rated = server.set_rating(ItemKind::Song, &song.id, 4).await;
    if !backend.supports_ratings() {
        assert!(matches!(rated, Err(ApiError::Unsupported(_))));
        return;
    }
    rated.expect("rate song");
    assert_eq!(
        server.song(&song.id).await.expect("song").user_rating,
        Some(4)
    );
    server
        .set_rating(ItemKind::Song, &song.id, 0)
        .await
        .expect("clear song rating");
    assert_eq!(server.song(&song.id).await.expect("song").user_rating, None);
    let album = album_named(&*server, "Afterglow").await;
    server
        .set_rating(ItemKind::Album, &album.id, 3)
        .await
        .expect("rate album");
    let rated_album = server.album(&album.id).await.expect("album").album;
    assert_eq!(rated_album.user_rating, Some(3));
    server
        .set_rating(ItemKind::Album, &album.id, 0)
        .await
        .expect("clear album rating");
}
