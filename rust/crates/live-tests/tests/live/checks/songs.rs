use feishin_api::MusicServer;
use feishin_core::model::{Paging, SongListQuery, SongSort, SortOrder};

use crate::support::{
    Backend, SONG_COUNT, SONG_TITLES, SONG_TITLES_BY_DURATION, album_named, all_songs,
    artist_named, everything, genre_id_named, song_titled, titles,
};

fn sorted(sort: SongSort, order: SortOrder) -> SongListQuery {
    SongListQuery {
        sort,
        order,
        ..SongListQuery::default()
    }
}

async fn sorted_titles(server: &dyn MusicServer, query: &SongListQuery) -> Vec<String> {
    let page = server.song_list(query, everything()).await.expect("songs");
    titles(&page.items).into_iter().map(str::to_owned).collect()
}

async fn filtered_titles(server: &dyn MusicServer, query: SongListQuery) -> Vec<String> {
    let mut found = sorted_titles(server, &query).await;
    found.sort();
    found
}

pub async fn pages_cover_the_library(backend: Backend) {
    let server = backend.connect().await;
    let query = SongListQuery::default();
    let mut collected = Vec::new();
    for start_index in (0..SONG_COUNT).step_by(3) {
        let page = server
            .song_list(
                &query,
                Paging {
                    start_index,
                    limit: 3,
                },
            )
            .await
            .expect("song page");
        assert_eq!(page.start_index, start_index);
        assert!(page.items.len() <= 3, "page holds {}", page.items.len());
        assert!(
            page.total_count.is_none_or(|total| total == SONG_COUNT),
            "{:?}",
            page.total_count
        );
        collected.extend(page.items.into_iter().map(|song| song.title));
    }
    if !backend.sorts_any_list() {
        collected.sort();
    }
    assert_eq!(collected, SONG_TITLES);
    assert_eq!(server.song_count(&query).await.expect("count"), SONG_COUNT);
}

pub async fn sorts_by_name_and_duration(backend: Backend) {
    let server = backend.connect().await;
    if !backend.sorts_any_list() {
        let unsorted = filtered_titles(&*server, SongListQuery::default()).await;
        assert_eq!(unsorted, SONG_TITLES);
        return;
    }
    let ascending = sorted_titles(&*server, &sorted(SongSort::Name, SortOrder::Asc)).await;
    assert_eq!(ascending, SONG_TITLES);
    let descending = sorted_titles(&*server, &sorted(SongSort::Name, SortOrder::Desc)).await;
    let mut reversed = SONG_TITLES.to_vec();
    reversed.reverse();
    assert_eq!(descending, reversed);
    let by_duration = sorted_titles(&*server, &sorted(SongSort::Duration, SortOrder::Asc)).await;
    assert_eq!(by_duration, SONG_TITLES_BY_DURATION);
}

pub async fn filters_by_album_and_artist(backend: Backend) {
    let server = backend.connect().await;
    let album = album_named(&*server, "Third Signal").await;
    let by_album = SongListQuery {
        album_ids: vec![album.id],
        ..SongListQuery::default()
    };
    assert_eq!(
        filtered_titles(&*server, by_album).await,
        ["Blue Note", "Late Set"]
    );
    let artist = artist_named(&*server, "Bravo Sine").await;
    let by_artist = SongListQuery {
        artist_ids: vec![artist.id],
        ..SongListQuery::default()
    };
    assert_eq!(
        filtered_titles(&*server, by_artist).await,
        ["Tidal Pull", "Undertow"]
    );
}

pub async fn filters_by_genre_and_text(backend: Backend) {
    let server = backend.connect().await;
    let by_genre = SongListQuery {
        genre_ids: vec![genre_id_named(&*server, "Jazz").await],
        ..SongListQuery::default()
    };
    assert_eq!(
        filtered_titles(&*server, by_genre).await,
        ["Blue Note", "Late Set"]
    );
    let by_text = SongListQuery {
        search: Some("Tidal".into()),
        ..SongListQuery::default()
    };
    assert_eq!(filtered_titles(&*server, by_text).await, ["Tidal Pull"]);
}

pub async fn song_details_match_the_tags(backend: Backend) {
    let server = backend.connect().await;
    let listed = song_titled(&*server, "Undertow").await;
    let song = server.song(&listed.id).await.expect("song");
    assert_eq!(song.title, "Undertow");
    assert_eq!(song.album.as_deref(), Some("Second Wave"));
    assert_eq!(song.year, Some(2021));
    assert_eq!(song.track_number, Some(2));
    assert_eq!(song.genres, ["Rock"]);
    assert!(
        song.artists
            .iter()
            .any(|artist| artist.name == "Bravo Sine")
    );
    assert!(
        (7_500..=8_500).contains(&song.duration_ms),
        "{}",
        song.duration_ms
    );
    assert!(song.album_id.is_some());
}

pub async fn random_and_similar_songs(backend: Backend) {
    let server = backend.connect().await;
    let random = server.random_songs(5, None).await.expect("random songs");
    assert_eq!(random.len(), 5);
    let rock_genre = genre_id_named(&*server, "Rock").await;
    let rock = server
        .random_songs(5, Some(&rock_genre))
        .await
        .expect("rock songs");
    let mut rock_titles = titles(&rock);
    rock_titles.sort_unstable();
    assert_eq!(rock_titles, ["Tidal Pull", "Undertow"]);
    let seed = all_songs(&*server).await.remove(0);
    let similar = server
        .similar_songs(&seed.id, 5)
        .await
        .expect("similar songs");
    assert!(similar.len() <= 5);
}

pub async fn song_media_fields_are_filled(backend: Backend) {
    let server = backend.connect().await;
    for (title, container) in [("Dawn Chorus", "mp3"), ("Tidal Pull", "flac")] {
        let listed = song_titled(&*server, title).await;
        let song = server.song(&listed.id).await.expect("song");
        assert_eq!(song.container.as_deref(), Some(container), "{title}");
        assert!(song.bitrate_kbps.is_some_and(|kbps| kbps > 0), "{title}");
        assert!(song.size_bytes.is_some_and(|size| size > 0), "{title}");
        assert!(song.created_at.is_some(), "{title}");
        assert!(!song.album_artists.is_empty(), "{title}");
        assert_eq!(listed.album_id, song.album_id, "{title}");
    }
    let covered = song_titled(&*server, "Dawn Chorus").await;
    assert!(covered.cover_art_id.is_some());
}
