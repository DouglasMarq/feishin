use std::path::PathBuf;
use std::sync::Arc;

use feishin_core::queue::QueueTrack;
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};

use super::command::forward_command;
use super::{Command, Controller, MAX_VOLUME, UiUpdate};

const MEDIA_COVER_SIZE: u32 = 512;

fn volume_percent(fraction: f64) -> u8 {
    (fraction.clamp(0.0, 1.0) * f64::from(MAX_VOLUME)).round() as u8
}

fn now_playing_of(
    track: &QueueTrack,
    cover_url: Option<String>,
    cover_file: Option<PathBuf>,
) -> NowPlaying {
    NowPlaying {
        track_id: track.song_id.to_string(),
        title: track.title.to_string(),
        artists: track
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect(),
        album: track.album.as_deref().map(str::to_owned),
        duration_ms: Some(track.duration_ms),
        cover_url,
        cover_file,
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
        if self.is_radio_playing() {
            self.stop_radio();
            return;
        }
        if self.playing {
            self.toggle_pause();
        }
        self.seek(0);
    }

    fn seek_by(&mut self, offset_ms: i64) {
        let target = self.position_ms.saturating_add_signed(offset_ms);
        self.seek(target.min(self.duration_ms));
    }

    fn media_cover_url(&self, track: &QueueTrack) -> Option<String> {
        if track.is_external() {
            return track.thumbnail_url().map(str::to_owned);
        }
        self.deps.session.get().and_then(|server| {
            track
                .cover_art_id
                .as_deref()
                .map(|cover_art_id| server.cover_art_url(cover_art_id, Some(MEDIA_COVER_SIZE)))
        })
    }

    fn media_cover_file(&self, track: &QueueTrack) -> Option<PathBuf> {
        if track.is_external() {
            return None;
        }
        let server = self.deps.session.get()?;
        let cover_art_id = track.cover_art_id.as_deref()?;
        self.deps
            .covers
            .cover_file(server.as_ref(), cover_art_id, MEDIA_COVER_SIZE)
    }

    fn cache_media_cover(&self, track: &QueueTrack) {
        if track.is_external() {
            return;
        }
        let (Some(server), Some(cover_art_id)) =
            (self.deps.session.get(), track.cover_art_id.as_deref())
        else {
            return;
        };
        let cover_art_id = cover_art_id.to_owned();
        let covers = Arc::clone(&self.deps.covers);
        let commands = self.deps.commands.clone();
        let song_id = track.song_id.to_string();
        tokio::spawn(async move {
            match covers
                .cover(server.as_ref(), &cover_art_id, MEDIA_COVER_SIZE)
                .await
            {
                Ok(_) => forward_command(&commands, Command::MediaCoverCached { song_id }),
                Err(error) => tracing::debug!("caching the media session cover failed: {error:#}"),
            }
        });
    }

    pub(super) fn update_media_metadata(&mut self, track: &QueueTrack) {
        let cover_file = self.media_cover_file(track);
        if cover_file.is_none() {
            self.cache_media_cover(track);
        }
        self.send_media_metadata(track, cover_file);
    }

    pub(super) fn on_media_cover_cached(&mut self, song_id: &str) {
        if self.is_radio_playing() {
            return;
        }
        let Some(track) = self
            .current_track()
            .filter(|track| track.song_id.as_ref() == song_id)
        else {
            return;
        };
        if let Some(cover_file) = self.media_cover_file(&track) {
            self.send_media_metadata(&track, Some(cover_file));
        }
    }

    fn send_media_metadata(&mut self, track: &QueueTrack, cover_file: Option<PathBuf>) {
        let cover_url = self.media_cover_url(track);
        let now_playing = now_playing_of(track, cover_url, cover_file);
        if let Err(error) = self.deps.media.set_metadata(&now_playing) {
            tracing::debug!("media session metadata: {error:#}");
        }
    }

    fn playback_status(&self) -> PlaybackStatus {
        match (self.loaded, self.playing) {
            (None, _) => PlaybackStatus::Stopped,
            (Some(_), true) => PlaybackStatus::Playing {
                position_ms: self.shown_position_ms(),
            },
            (Some(_), false) => PlaybackStatus::Paused {
                position_ms: self.shown_position_ms(),
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
        let volume = match playback.is_muted {
            true => 0,
            false => playback.volume_percent,
        };
        if let Err(error) = self
            .deps
            .media
            .set_volume(f64::from(volume) / f64::from(MAX_VOLUME))
        {
            tracing::debug!("media session volume: {error:#}");
        }
    }
}
