use std::collections::{HashMap, HashSet};

use feishin_core::model::Song;

use crate::discover::names::normalized_name;

const FAVORITE_BONUS: u64 = 5;
const LISTED_ARTISTS: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct SeedCluster {
    pub artists: Vec<String>,
    pub seeds: Vec<Song>,
}

pub fn rank_seeds(most_played: Vec<Song>, favorites: Vec<Song>) -> Vec<Song> {
    let favorite_ids: HashSet<String> = favorites.iter().map(|song| song.id.clone()).collect();
    let mut seen = HashSet::new();
    let mut ranked: Vec<(u64, Song)> = most_played
        .into_iter()
        .chain(favorites)
        .filter(|song| seen.insert(song.id.clone()))
        .map(|song| (seed_score(&song, &favorite_ids), song))
        .collect();
    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.last_played_at.cmp(&left.last_played_at))
    });
    ranked.into_iter().map(|(_score, song)| song).collect()
}

fn seed_score(song: &Song, favorite_ids: &HashSet<String>) -> u64 {
    let favorite = song.favorite || favorite_ids.contains(&song.id);
    u64::from(song.play_count) + if favorite { FAVORITE_BONUS } else { 0 }
}

pub fn primary_artist(song: &Song) -> Option<&str> {
    song.artists
        .first()
        .or_else(|| song.album_artists.first())
        .map(|artist| artist.name.as_str())
        .filter(|name| !name.trim().is_empty())
}

pub fn cluster_seeds(
    ranked: &[Song],
    max_clusters: usize,
    seeds_per_cluster: usize,
) -> Vec<SeedCluster> {
    let mut order: Vec<String> = Vec::new();
    let mut members: HashMap<String, Vec<&Song>> = HashMap::new();
    for song in ranked {
        let Some(key) = cluster_key(song) else {
            continue;
        };
        if !members.contains_key(&key) {
            order.push(key.clone());
        }
        members.entry(key).or_default().push(song);
    }
    order
        .iter()
        .filter_map(|key| members.get(key))
        .take(max_clusters)
        .map(|songs| cluster_of(songs, seeds_per_cluster))
        .collect()
}

fn cluster_key(song: &Song) -> Option<String> {
    song.genres
        .iter()
        .map(|genre| normalized_name(genre))
        .find(|genre| !genre.is_empty())
        .or_else(|| primary_artist(song).map(normalized_name))
        .filter(|key| !key.is_empty())
}

fn cluster_of(songs: &[&Song], seeds_per_cluster: usize) -> SeedCluster {
    let mut seen_artists = HashSet::new();
    let (distinct, repeated): (Vec<&Song>, Vec<&Song>) = songs.iter().copied().partition(|song| {
        seen_artists.insert(
            primary_artist(song)
                .map(normalized_name)
                .unwrap_or_default(),
        )
    });
    let seeds: Vec<Song> = distinct
        .into_iter()
        .chain(repeated)
        .take(seeds_per_cluster)
        .cloned()
        .collect();
    let mut listed = HashSet::new();
    let artists = songs
        .iter()
        .filter_map(|song| primary_artist(song))
        .filter(|artist| listed.insert(normalized_name(artist)))
        .take(LISTED_ARTISTS)
        .map(str::to_owned)
        .collect();
    SeedCluster { artists, seeds }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ArtistRef;

    use super::*;
    use crate::demo::test_song;

    fn song(id: &str, artist: &str, genre: Option<&str>, play_count: u32) -> Song {
        Song {
            title: format!("Song {id}"),
            artists: vec![ArtistRef {
                id: format!("artist-{artist}"),
                name: artist.to_owned(),
            }],
            genres: genre.map(str::to_owned).into_iter().collect(),
            play_count,
            ..test_song(id, 200_000)
        }
    }

    fn ids(songs: &[Song]) -> Vec<&str> {
        songs.iter().map(|song| song.id.as_str()).collect()
    }

    #[test]
    fn favorites_count_extra_and_duplicates_are_dropped() {
        let most_played = vec![song("a", "A", None, 4), song("b", "B", None, 3)];
        let favorites = vec![song("b", "B", None, 3), song("c", "C", None, 0)];
        let ranked = rank_seeds(most_played, favorites);
        assert_eq!(ids(&ranked), ["b", "c", "a"]);
    }

    #[test]
    fn ties_go_to_the_song_played_most_recently() {
        let older = Song {
            last_played_at: Some("2026-01-01T00:00:00Z".into()),
            ..song("old", "A", None, 2)
        };
        let newer = Song {
            last_played_at: Some("2026-10-01T00:00:00Z".into()),
            ..song("new", "B", None, 2)
        };
        assert_eq!(
            ids(&rank_seeds(vec![older, newer], Vec::new())),
            ["new", "old"]
        );
    }

    #[test]
    fn clusters_follow_genres_then_artists_and_prefer_different_artists() {
        let ranked = vec![
            song("1", "Daft Punk", Some("Electronic"), 9),
            song("2", "Daft Punk", Some("electronic"), 8),
            song("3", "Justice", Some("Electronic"), 7),
            song("4", "Cartola", Some("Samba"), 6),
            song("5", "Untagged Band", None, 5),
            song("6", "Air", Some("Electronic"), 4),
        ];
        let clusters = cluster_seeds(&ranked, 3, 3);
        assert_eq!(clusters.len(), 3);
        let electronic = clusters.first().unwrap();
        assert_eq!(ids(&electronic.seeds), ["1", "3", "6"]);
        assert_eq!(electronic.artists, ["Daft Punk", "Justice", "Air"]);
        assert_eq!(ids(&clusters.get(1).unwrap().seeds), ["4"]);
        assert_eq!(clusters.get(2).unwrap().artists, ["Untagged Band"]);
        assert_eq!(cluster_seeds(&ranked, 1, 3).len(), 1);
    }
}
