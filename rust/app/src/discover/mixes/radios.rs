use std::collections::HashMap;
use std::sync::Arc;

use feishin_core::model::Song;
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::MusicSong;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::seeds::primary_artist;
use crate::discover::DiscoverSource;
use crate::discover::library_match::is_confident_match;
use crate::discover::names::search_query;

const PARALLEL_SEEDS: usize = 3;
const RADIO_SONGS_PER_SEED: usize = 25;

#[derive(Debug, Clone, PartialEq)]
pub struct SeedRadio {
    pub seed: MusicSong,
    pub radio: Vec<MusicSong>,
}

pub async fn seed_radios(
    source: Arc<dyn DiscoverSource>,
    settings: &YouTubeSettings,
    seeds: &[Song],
) -> HashMap<String, SeedRadio> {
    let permits = Arc::new(Semaphore::new(PARALLEL_SEEDS));
    let mut tasks = JoinSet::new();
    for seed in seeds {
        let (source, permits, settings, seed) = (
            Arc::clone(&source),
            Arc::clone(&permits),
            settings.clone(),
            seed.clone(),
        );
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.ok();
            let radio = seed_radio(source.as_ref(), &settings, &seed).await;
            (seed.id, radio)
        });
    }
    let mut radios = HashMap::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((seed_id, Some(radio))) => {
                radios.insert(seed_id, radio);
            }
            Ok((_seed_id, None)) => {}
            Err(error) => tracing::debug!("a mix seed lookup stopped: {error}"),
        }
    }
    radios
}

async fn seed_radio(
    source: &dyn DiscoverSource,
    settings: &YouTubeSettings,
    seed: &Song,
) -> Option<SeedRadio> {
    let found = seed_video(source, settings, seed).await?;
    match source.radio(found.video_id.clone(), settings.clone()).await {
        Ok(radio) => Some(SeedRadio {
            radio: radio.into_iter().take(RADIO_SONGS_PER_SEED).collect(),
            seed: found,
        }),
        Err(error) => {
            tracing::debug!("the YouTube Music radio for a mix seed failed: {error}");
            None
        }
    }
}

async fn seed_video(
    source: &dyn DiscoverSource,
    settings: &YouTubeSettings,
    seed: &Song,
) -> Option<MusicSong> {
    let title = search_query(&seed.title);
    let with_artist = primary_artist(seed).map(|artist| format!("{title} {artist}"));
    for query in with_artist.into_iter().chain([title.clone()]) {
        match source.search(query, settings.clone()).await {
            Ok(results) => {
                if let Some(found) = results
                    .into_iter()
                    .find(|result| is_confident_match(result, seed))
                {
                    return Some(found);
                }
            }
            Err(error) => tracing::debug!("searching YouTube Music for a mix seed failed: {error}"),
        }
    }
    None
}
