use feishin_core::queue::{Uid, YouTubeVideo};
use feishin_youtube::{StreamUrl, YouTubeError};

use super::ResolvePurpose;
use crate::controller::Controller;

impl Controller {
    pub(in crate::controller) fn prepare_youtube_preload(
        &mut self,
        uid: Uid,
        video: &YouTubeVideo,
    ) {
        if !self.youtube.wants_preload(uid) {
            return;
        }
        let task = self.spawn_resolution(uid, &video.video_id, ResolvePurpose::Preload);
        self.youtube.track_preload(uid, task);
    }

    pub(super) fn finish_preload_resolution(
        &mut self,
        uid: Uid,
        result: Result<StreamUrl, YouTubeError>,
    ) {
        self.youtube.finish_preload(uid);
        match result {
            Ok(_) => self.refresh_preload(),
            Err(error) => {
                tracing::warn!("preparing the next YouTube track failed: {error}");
                self.youtube.preload_failed(uid);
            }
        }
    }
}
