use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::MusicSong;

use super::calendar::MixDay;
use super::compose::{Candidate, compose_daily, compose_weekly};
use super::library_seeds::library_seeds;
use super::model::{DailyMix, MIX_FORMAT_VERSION, MixSet, WeeklyMix};
use super::radios::{SeedRadio, seed_radios};
use super::seeds::{SeedCluster, cluster_seeds, primary_artist};
use crate::discover::names::normalized_name;
use crate::discover::{DiscoverSource, LibraryMatcher, LibraryPresence};

const MAX_DAILY_MIXES: usize = 4;
const SEEDS_PER_MIX: usize = 3;
const WEEKLY_SEEDS: usize = 6;
const MIN_MIX_LENGTH: usize = 12;
const PRESENCE_BUDGET: Duration = Duration::from_secs(30);

pub struct MixInputs {
    pub server: Arc<dyn MusicServer>,
    pub source: Arc<dyn DiscoverSource>,
    pub settings: YouTubeSettings,
    pub matcher: LibraryMatcher,
    pub today: MixDay,
    pub previous: Option<MixSet>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixBuildError {
    EmptyLibrary,
    NoRadio,
}

impl fmt::Display for MixBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLibrary => {
                formatter.write_str("the library has no songs to start mixes from")
            }
            Self::NoRadio => formatter.write_str("YouTube Music had no radio for any seed song"),
        }
    }
}

pub async fn build_mixes(inputs: MixInputs) -> Result<MixSet, MixBuildError> {
    let ranked = library_seeds(inputs.server.as_ref()).await;
    if ranked.is_empty() {
        return Err(MixBuildError::EmptyLibrary);
    }
    let clusters = cluster_seeds(&ranked, MAX_DAILY_MIXES, SEEDS_PER_MIX);
    let weekly_seeds = distinct_artist_seeds(&ranked, WEEKLY_SEEDS);
    let all_seeds = unique_seeds(
        clusters
            .iter()
            .flat_map(|cluster| &cluster.seeds)
            .chain(&weekly_seeds),
    );
    let radios = seed_radios(Arc::clone(&inputs.source), &inputs.settings, &all_seeds).await;
    if radios.is_empty() {
        return Err(MixBuildError::NoRadio);
    }
    let presence = library_presence(&inputs, &radios).await;
    let server_id = inputs.server.config().id.clone();
    Ok(MixSet {
        version: MIX_FORMAT_VERSION,
        daily: daily_mixes(&clusters, &radios, &presence, inputs.today),
        weekly: weekly_mix(&inputs, &server_id, &weekly_seeds, &radios, &presence),
        server_id,
        day: inputs.today,
    })
}

fn distinct_artist_seeds(ranked: &[Song], count: usize) -> Vec<Song> {
    let mut artists = HashSet::new();
    ranked
        .iter()
        .filter(|song| {
            artists.insert(
                primary_artist(song)
                    .map(normalized_name)
                    .unwrap_or_default(),
            )
        })
        .take(count)
        .cloned()
        .collect()
}

fn unique_seeds<'seed>(seeds: impl Iterator<Item = &'seed Song>) -> Vec<Song> {
    let mut seen = HashSet::new();
    seeds
        .filter(|song| seen.insert(song.id.clone()))
        .cloned()
        .collect()
}

async fn library_presence(
    inputs: &MixInputs,
    radios: &HashMap<String, SeedRadio>,
) -> HashMap<String, LibraryPresence> {
    let mut seen = HashSet::new();
    let candidates: Vec<MusicSong> = radios
        .values()
        .flat_map(|radio| &radio.radio)
        .filter(|song| seen.insert(song.video_id.clone()))
        .cloned()
        .collect();
    let presence = inputs
        .matcher
        .presence(Arc::clone(&inputs.server), &candidates, PRESENCE_BUDGET)
        .await;
    candidates
        .into_iter()
        .map(|song| song.video_id)
        .zip(presence)
        .collect()
}

fn candidates_of(radio: &SeedRadio, presence: &HashMap<String, LibraryPresence>) -> Vec<Candidate> {
    radio
        .radio
        .iter()
        .map(|song| Candidate {
            presence: presence
                .get(&song.video_id)
                .copied()
                .unwrap_or(LibraryPresence::Unknown),
            song: song.clone(),
        })
        .collect()
}

fn daily_mixes(
    clusters: &[SeedCluster],
    radios: &HashMap<String, SeedRadio>,
    presence: &HashMap<String, LibraryPresence>,
    today: MixDay,
) -> Vec<DailyMix> {
    let composed = clusters.iter().enumerate().filter_map(|(index, cluster)| {
        let cluster_radios: Vec<&SeedRadio> = cluster
            .seeds
            .iter()
            .filter_map(|seed| radios.get(&seed.id))
            .collect();
        let seed_songs: Vec<MusicSong> = cluster_radios
            .iter()
            .map(|radio| radio.seed.clone())
            .collect();
        let candidates: Vec<Candidate> = cluster_radios
            .iter()
            .flat_map(|radio| candidates_of(radio, presence))
            .collect();
        let songs = compose_daily(&seed_songs, &candidates, today.shuffle_seed(index as u64));
        (songs.len() >= MIN_MIX_LENGTH).then(|| (cluster.artists.clone(), songs))
    });
    composed
        .zip(1_u8..)
        .map(|((artists, songs), number)| DailyMix {
            number,
            artists,
            songs,
        })
        .collect()
}

fn weekly_mix(
    inputs: &MixInputs,
    server_id: &str,
    weekly_seeds: &[Song],
    radios: &HashMap<String, SeedRadio>,
    presence: &HashMap<String, LibraryPresence>,
) -> Option<WeeklyMix> {
    let week = inputs.today.week();
    let previous = inputs
        .previous
        .as_ref()
        .filter(|previous| previous.is_for_server(server_id));
    if let Some(kept) = previous.and_then(|previous| previous.weekly_for(server_id, week)) {
        return Some(kept.clone());
    }
    let excluded: HashSet<String> = previous
        .and_then(|previous| previous.weekly.as_ref())
        .map(|weekly| {
            weekly
                .songs
                .iter()
                .map(|song| song.video_id.clone())
                .collect()
        })
        .unwrap_or_default();
    let seed_radios: Vec<Vec<Candidate>> = weekly_seeds
        .iter()
        .filter_map(|seed| radios.get(&seed.id))
        .map(|radio| candidates_of(radio, presence))
        .collect();
    let songs = compose_weekly(&seed_radios, &excluded);
    (songs.len() >= MIN_MIX_LENGTH).then_some(WeeklyMix { week, songs })
}
