use feishin_core::queue::Uid;
use feishin_storage::settings::YouTubeSettings;
use tokio::task::AbortHandle;

use super::{ResolvePurpose, YouTubeResolution};
use crate::controller::{Command, Controller, YouTubeCommand, forward_command};
use crate::youtube::yt_dlp_for;

impl Controller {
    pub(super) fn spawn_resolution(
        &self,
        uid: Uid,
        video_id: &str,
        purpose: ResolvePurpose,
    ) -> AbortHandle {
        let yt_dlp = yt_dlp_for(&self.settings.youtube);
        let commands = self.deps.commands.clone();
        let video_id = Box::<str>::from(video_id);
        tokio::spawn(async move {
            let result = yt_dlp.audio_stream(&video_id).await;
            let resolution = YouTubeResolution {
                uid,
                video_id,
                purpose,
                result,
            };
            forward_command(
                &commands,
                Command::YouTube(YouTubeCommand::Resolved(resolution)),
            );
        })
        .abort_handle()
    }

    pub(in crate::controller) fn on_youtube_command(&mut self, command: YouTubeCommand) {
        match command {
            YouTubeCommand::Resolved(resolution) => self.on_youtube_resolved(resolution),
            YouTubeCommand::SaveSettings(settings) => self.save_youtube_settings(settings),
        }
    }

    fn on_youtube_resolved(&mut self, resolution: YouTubeResolution) {
        let YouTubeResolution {
            uid,
            video_id,
            purpose,
            result,
        } = resolution;
        if let Ok(stream) = &result {
            self.youtube.remember(&video_id, stream.clone());
        }
        match purpose {
            ResolvePurpose::Load { request, start_ms } => {
                self.finish_load_resolution(uid, request, start_ms, result);
            }
            ResolvePurpose::Preload => self.finish_preload_resolution(uid, result),
        }
    }

    fn save_youtube_settings(&mut self, settings: YouTubeSettings) {
        self.settings.youtube = settings;
        self.youtube.forget_failed_preload();
        self.save_settings();
        self.refresh_preload();
    }

    pub(in crate::controller) fn forget_failed_stream(&mut self, uid: Option<Uid>) {
        if uid.is_none() || uid != self.loaded {
            return;
        }
        let video_id = self
            .queue
            .current()
            .and_then(|item| item.track.youtube_video())
            .map(|video| video.video_id.clone());
        if let Some(video_id) = video_id {
            self.youtube.forget(&video_id);
        }
    }
}
