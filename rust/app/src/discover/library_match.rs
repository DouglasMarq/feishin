use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_youtube::music::MusicSong;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::names::{normalized_name, search_query};

const SEARCH_LIMIT: usize = 25;
const DURATION_TOLERANCE_MS: u64 = 4_000;
const PARALLEL_SEARCHES: usize = 6;
const MATCH_BUDGET: Duration = Duration::from_secs(8);
const MAX_REMEMBERED: usize = 2_000;

type MatchKey = (String, String);
type SearchOutcome = (usize, String, Option<Song>);

pub fn is_confident_match(song: &MusicSong, candidate: &Song) -> bool {
    normalized_name(&song.title) == normalized_name(&candidate.title)
        && shares_artist(song, candidate)
        && durations_agree(song.duration_ms, candidate.duration_ms)
}

fn shares_artist(song: &MusicSong, candidate: &Song) -> bool {
    let candidate_names: Vec<String> = candidate
        .artists
        .iter()
        .chain(&candidate.album_artists)
        .map(|artist| normalized_name(&artist.name))
        .collect();
    let credited_together = normalized_name(
        &candidate
            .artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    song.artists
        .iter()
        .map(|artist| normalized_name(artist))
        .filter(|artist| !artist.is_empty())
        .any(|wanted| candidate_names.contains(&wanted) || credited_together == wanted)
}

fn durations_agree(youtube_ms: Option<u64>, library_ms: u64) -> bool {
    match youtube_ms {
        Some(youtube_ms) if library_ms > 0 => {
            youtube_ms.abs_diff(library_ms) <= DURATION_TOLERANCE_MS
        }
        _ => true,
    }
}

pub async fn library_match(server: &dyn MusicServer, song: &MusicSong) -> Option<Song> {
    let query = search_query(&song.title);
    if query.is_empty() {
        return None;
    }
    match server.search(&query, SEARCH_LIMIT).await {
        Ok(results) => results
            .songs
            .into_iter()
            .find(|candidate| is_confident_match(song, candidate)),
        Err(error) => {
            tracing::debug!("library search for a YouTube song failed: {error}");
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPresence {
    InLibrary,
    NotInLibrary,
    Unknown,
}

#[derive(Clone, Default)]
pub struct LibraryMatcher {
    remembered: Arc<Mutex<HashMap<MatchKey, Option<Song>>>>,
}

impl LibraryMatcher {
    pub async fn matches(
        &self,
        server: Arc<dyn MusicServer>,
        songs: &[MusicSong],
    ) -> Vec<Option<Song>> {
        self.lookup(server, songs, MATCH_BUDGET)
            .await
            .into_iter()
            .map(Option::flatten)
            .collect()
    }

    pub async fn presence(
        &self,
        server: Arc<dyn MusicServer>,
        songs: &[MusicSong],
        budget: Duration,
    ) -> Vec<LibraryPresence> {
        self.lookup(server, songs, budget)
            .await
            .into_iter()
            .map(|looked_up| match looked_up {
                Some(Some(_)) => LibraryPresence::InLibrary,
                Some(None) => LibraryPresence::NotInLibrary,
                None => LibraryPresence::Unknown,
            })
            .collect()
    }

    async fn lookup(
        &self,
        server: Arc<dyn MusicServer>,
        songs: &[MusicSong],
        budget: Duration,
    ) -> Vec<Option<Option<Song>>> {
        let server_id = server.config().id.clone();
        let mut found: Vec<Option<Option<Song>>> = vec![None; songs.len()];
        let mut pending = Vec::new();
        for ((index, song), slot) in songs.iter().enumerate().zip(found.iter_mut()) {
            match self.remembered(&server_id, &song.video_id) {
                Some(known) => *slot = Some(known),
                None => pending.push((index, song.clone())),
            }
        }
        for (index, video_id, matched) in search_all(server, pending, budget).await {
            self.remember(&server_id, video_id, matched.clone());
            if let Some(slot) = found.get_mut(index) {
                *slot = Some(matched);
            }
        }
        found
    }

    fn remembered(&self, server_id: &str, video_id: &str) -> Option<Option<Song>> {
        let remembered = self.remembered.lock().ok()?;
        remembered
            .get(&(server_id.to_owned(), video_id.to_owned()))
            .cloned()
    }

    fn remember(&self, server_id: &str, video_id: String, matched: Option<Song>) {
        let Ok(mut remembered) = self.remembered.lock() else {
            return;
        };
        if remembered.len() >= MAX_REMEMBERED {
            remembered.clear();
        }
        remembered.insert((server_id.to_owned(), video_id), matched);
    }
}

async fn search_all(
    server: Arc<dyn MusicServer>,
    pending: Vec<(usize, MusicSong)>,
    budget: Duration,
) -> Vec<SearchOutcome> {
    let permits = Arc::new(Semaphore::new(PARALLEL_SEARCHES));
    let mut tasks = JoinSet::new();
    for (index, song) in pending {
        let (server, permits) = (Arc::clone(&server), Arc::clone(&permits));
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.ok();
            let matched = library_match(server.as_ref(), &song).await;
            (index, song.video_id, matched)
        });
    }
    let mut outcomes = Vec::new();
    let collecting = async {
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => tracing::debug!("a library search stopped: {error}"),
            }
        }
    };
    if tokio::time::timeout(budget, collecting).await.is_err() {
        tracing::debug!("library matching ran out of time; the rest plays from YouTube");
    }
    outcomes
}
