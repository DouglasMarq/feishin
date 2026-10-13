use feishin_core::model::{ArtistListQuery, ArtistSort, Paging, SortOrder};

use crate::support::{ARTIST_NAMES, Backend, album_names, artist_named, artist_names, everything};

pub async fn lists_sorts_and_pages_artists(backend: Backend) {
    let server = backend.connect().await;
    let ascending = server
        .artist_list(&ArtistListQuery::default(), everything())
        .await
        .expect("artists");
    assert_eq!(artist_names(&ascending.items), ARTIST_NAMES);
    let descending_query = ArtistListQuery {
        sort: ArtistSort::Name,
        order: SortOrder::Desc,
        ..ArtistListQuery::default()
    };
    let descending = server
        .artist_list(&descending_query, everything())
        .await
        .expect("artists descending");
    let mut reversed = ARTIST_NAMES.to_vec();
    reversed.reverse();
    assert_eq!(artist_names(&descending.items), reversed);
    let second_page = server
        .artist_list(
            &ArtistListQuery::default(),
            Paging {
                start_index: 1,
                limit: 1,
            },
        )
        .await
        .expect("artist page");
    assert_eq!(artist_names(&second_page.items), ["Bravo Sine"]);
}

pub async fn artist_detail_lists_albums(backend: Backend) {
    let server = backend.connect().await;
    let listed = artist_named(&*server, "Aurora Tones").await;
    let listed_count = listed.album_count;
    assert!(
        listed_count.is_none_or(|count| count == 2),
        "{listed_count:?}"
    );
    if backend.counts_albums_in_artist_lists() {
        assert_eq!(listed_count, Some(2));
    }
    let detail = server.artist(&listed.id).await.expect("artist detail");
    assert_eq!(detail.artist.name, "Aurora Tones");
    assert_eq!(detail.artist.album_count, Some(2));
    let mut albums = album_names(&detail.albums);
    albums.sort_unstable();
    assert_eq!(albums, ["Afterglow", "First Light"]);
}
