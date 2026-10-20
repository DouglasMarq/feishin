use std::collections::HashSet;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, Song, SongListQuery, SongSort, SortOrder};

use super::seeds::rank_seeds;

const MOST_PLAYED_LIMIT: usize = 200;
const FAVORITES_LIMIT: usize = 100;
const FALLBACK_LIMIT: usize = 50;
const ENOUGH_SEEDS: usize = 5;

pub async fn library_seeds(server: &dyn MusicServer) -> Vec<Song> {
    let most_played: Vec<Song> = listed(server, sorted(SongSort::PlayCount), MOST_PLAYED_LIMIT)
        .await
        .into_iter()
        .filter(|song| song.play_count > 0)
        .collect();
    let favorites = listed(server, favorites_query(), FAVORITES_LIMIT).await;
    let mut seeds = rank_seeds(most_played, favorites);
    if seeds.len() < ENOUGH_SEEDS {
        let recently_played = listed(server, sorted(SongSort::RecentlyPlayed), FALLBACK_LIMIT)
            .await
            .into_iter()
            .filter(|song| song.last_played_at.is_some());
        append_unique(&mut seeds, recently_played);
    }
    if seeds.len() < ENOUGH_SEEDS {
        append_unique(&mut seeds, random(server).await);
    }
    seeds
}

fn sorted(sort: SongSort) -> SongListQuery {
    SongListQuery {
        sort,
        order: SortOrder::Desc,
        ..SongListQuery::default()
    }
}

fn favorites_query() -> SongListQuery {
    SongListQuery {
        is_favorite: Some(true),
        ..sorted(SongSort::Favorited)
    }
}

async fn listed(server: &dyn MusicServer, query: SongListQuery, limit: usize) -> Vec<Song> {
    let paging = Paging {
        start_index: 0,
        limit,
    };
    match server.song_list(&query, paging).await {
        Ok(page) => page.items,
        Err(error) => {
            tracing::debug!("listing songs for the mixes failed: {error}");
            Vec::new()
        }
    }
}

async fn random(server: &dyn MusicServer) -> Vec<Song> {
    server
        .random_songs(FALLBACK_LIMIT, None)
        .await
        .inspect_err(|error| tracing::debug!("random songs for the mixes failed: {error}"))
        .unwrap_or_default()
}

fn append_unique(seeds: &mut Vec<Song>, more: impl IntoIterator<Item = Song>) {
    let mut known: HashSet<String> = seeds.iter().map(|song| song.id.clone()).collect();
    seeds.extend(
        more.into_iter()
            .filter(|song| known.insert(song.id.clone())),
    );
}
