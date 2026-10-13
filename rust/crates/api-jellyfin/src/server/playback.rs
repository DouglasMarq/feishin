use feishin_api::Result;
use reqwest::Method;

use super::JellyfinServer;
use crate::dto;
use crate::normalize::TICKS_PER_MS;
use crate::query_params::QueryParams;

const NOW_PLAYING_PATH: &str = "sessions/playing";
const PROGRESS_PATH: &str = "sessions/playing/progress";
const STOPPED_PATH: &str = "sessions/playing/stopped";

impl JellyfinServer {
    pub async fn report_progress(
        &self,
        song_id: &str,
        position_ms: u64,
        paused: bool,
    ) -> Result<()> {
        let report = dto::PlaybackReport {
            item_id: song_id,
            event_name: Some(dto::PlaybackEvent::for_paused_state(paused)),
            is_paused: Some(paused),
            position_ticks: Some(position_ms * TICKS_PER_MS),
        };
        self.post_playback_report(PROGRESS_PATH, &report).await
    }

    pub async fn report_stopped(&self, song_id: &str, position_ms: u64) -> Result<()> {
        let report = dto::PlaybackReport {
            item_id: song_id,
            event_name: None,
            is_paused: None,
            position_ticks: Some(position_ms * TICKS_PER_MS),
        };
        self.post_playback_report(STOPPED_PATH, &report).await
    }

    pub(super) async fn report_now_playing(&self, song_id: &str) -> Result<()> {
        let report = dto::PlaybackReport {
            item_id: song_id,
            event_name: None,
            is_paused: None,
            position_ticks: Some(0),
        };
        self.post_playback_report(NOW_PLAYING_PATH, &report).await
    }

    pub(super) async fn report_played_to_end(&self, song_id: &str) -> Result<()> {
        let report = dto::PlaybackReport {
            item_id: song_id,
            event_name: None,
            is_paused: Some(true),
            position_ticks: self.song_duration_ticks(song_id).await,
        };
        self.post_playback_report(STOPPED_PATH, &report).await
    }

    pub(super) async fn mark_favorites(&self, ids: &[String], favorite: bool) -> Result<()> {
        let method = match favorite {
            true => Method::POST,
            false => Method::DELETE,
        };
        for id in ids {
            let path = format!("users/{}/favoriteitems/{id}", self.user_id);
            let request = self.request(method.clone(), &path).json(&dto::EmptyBody {});
            self.execute(request).await?;
        }
        Ok(())
    }

    async fn song_duration_ticks(&self, song_id: &str) -> Option<u64> {
        self.get::<dto::Song>(&self.item_path(song_id), &QueryParams::default())
            .await
            .ok()
            .and_then(|song| song.run_time_ticks)
    }

    async fn post_playback_report(
        &self,
        path: &str,
        report: &dto::PlaybackReport<'_>,
    ) -> Result<()> {
        self.execute(self.request(Method::POST, path).json(report))
            .await
    }
}
