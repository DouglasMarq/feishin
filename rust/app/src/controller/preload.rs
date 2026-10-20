use super::Controller;
use super::playback::Playable;
use crate::audio::Track;

impl Controller {
    pub(super) fn refresh_preload(&mut self) {
        if !self.preloads_next_track() || self.loaded.is_none() || self.is_radio_playing() {
            return;
        }
        let candidate = self
            .queue
            .peek_next()
            .filter(|_| !self.sleep_timer.ends_with_track())
            .map(|item| (item.uid, self.playable(item)));
        let next = match candidate {
            Some((_, Playable::Ready(track))) => Some(track),
            Some((uid, Playable::NeedsResolution(video))) => {
                self.prepare_youtube_preload(uid, &video);
                None
            }
            Some((_, Playable::NotConnected)) | None => None,
        };
        self.preload(next);
    }

    fn preload(&mut self, next: Option<Track>) {
        if next.as_ref().map(|track| track.uid) == self.preloaded {
            return;
        }
        let result = match &next {
            Some(track) => self.deps.audio.preload_next(track),
            None => self.deps.audio.clear_next(),
        };
        match result {
            Ok(()) => self.preloaded = next.map(|track| track.uid),
            Err(error) => tracing::warn!("preloading failed: {error:#}"),
        }
    }

    fn preloads_next_track(&self) -> bool {
        let playback = &self.settings.playback;
        playback.is_gapless || !playback.crossfade().is_zero()
    }

    pub(super) fn discard_preload(&mut self) {
        if self.preloaded.take().is_some()
            && let Err(error) = self.deps.audio.clear_next()
        {
            tracing::warn!("clearing the preload failed: {error:#}");
        }
    }
}
