use feishin_core::model::Song;
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};

use super::{Controller, UiUpdate};

const MEDIA_COVER_SIZE: u32 = 512;

fn volume_percent(fraction: f64) -> u8 {
    (fraction.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn now_playing_of(song: &Song, cover_url: Option<String>) -> NowPlaying {
    NowPlaying {
        track_id: song.id.clone(),
        title: song.title.clone(),
        artists: song
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect(),
        album: song.album.clone(),
        duration_ms: Some(song.duration_ms),
        cover_url,
    }
}

impl Controller {
    pub(super) fn on_media_event(&mut self, event: MediaControlEvent) {
        match event {
            MediaControlEvent::Play if !self.playing => self.toggle_pause(),
            MediaControlEvent::Pause if self.playing => self.toggle_pause(),
            MediaControlEvent::Play | MediaControlEvent::Pause => {}
            MediaControlEvent::Toggle => self.toggle_pause(),
            MediaControlEvent::Next => self.next(),
            MediaControlEvent::Previous => self.previous(),
            MediaControlEvent::Stop => self.stop_from_media_session(),
            MediaControlEvent::SeekTo { position_ms } => self.seek(position_ms),
            MediaControlEvent::SeekBy { offset_ms } => self.seek_by(offset_ms),
            MediaControlEvent::SetVolume(fraction) => self.set_volume(volume_percent(fraction)),
            MediaControlEvent::Raise => self.deps.ui.send(UiUpdate::Raise),
            MediaControlEvent::Quit => self.deps.ui.send(UiUpdate::Quit),
        }
    }

    fn stop_from_media_session(&mut self) {
        if self.playing {
            self.toggle_pause();
        }
        self.seek(0);
    }

    fn seek_by(&mut self, offset_ms: i64) {
        let target = self.position_ms.saturating_add_signed(offset_ms);
        self.seek(target.min(self.duration_ms));
    }

    pub(super) fn update_media_metadata(&mut self, song: &Song) {
        let cover_url = self.deps.session.get().and_then(|server| {
            song.cover_art_id
                .as_deref()
                .map(|cover_art_id| server.cover_art_url(cover_art_id, Some(MEDIA_COVER_SIZE)))
        });
        if let Err(error) = self
            .deps
            .media
            .set_metadata(&now_playing_of(song, cover_url))
        {
            tracing::debug!("media session metadata: {error:#}");
        }
    }

    fn playback_status(&self) -> PlaybackStatus {
        match (self.loaded, self.playing) {
            (None, _) => PlaybackStatus::Stopped,
            (Some(_), true) => PlaybackStatus::Playing {
                position_ms: self.position_ms,
            },
            (Some(_), false) => PlaybackStatus::Paused {
                position_ms: self.position_ms,
            },
        }
    }

    pub(super) fn update_media_status(&mut self) {
        let status = self.playback_status();
        if let Err(error) = self.deps.media.set_status(status) {
            tracing::debug!("media session status: {error:#}");
        }
    }

    pub(super) fn update_media_volume(&mut self) {
        let playback = &self.settings.playback;
        let volume = match playback.muted {
            true => 0,
            false => playback.volume,
        };
        if let Err(error) = self.deps.media.set_volume(f64::from(volume) / 100.0) {
            tracing::debug!("media session volume: {error:#}");
        }
    }
}
