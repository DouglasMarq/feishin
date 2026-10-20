use std::time::Duration;

use souvlaki::{
    MediaControlEvent as SystemEvent, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

use crate::{MediaControlEvent, MediaIdentity, NowPlaying, PlatformError, PlaybackStatus, Result};

const SEEK_STEP_MS: i64 = 10_000;

pub struct MediaControls {
    controls: souvlaki::MediaControls,
    status: PlaybackStatus,
}

impl MediaControls {
    pub fn new(
        identity: MediaIdentity<'_>,
        on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>,
    ) -> Result<Self> {
        let config = PlatformConfig {
            display_name: identity.name,
            dbus_name: identity.desktop_entry,
            hwnd: None,
        };
        let mut controls = souvlaki::MediaControls::new(config).map_err(media_error)?;
        controls
            .attach(move |event| {
                if let Some(event) = media_event_from(event) {
                    on_event(event);
                }
            })
            .map_err(media_error)?;
        Ok(Self {
            controls,
            status: PlaybackStatus::Stopped,
        })
    }

    pub fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        let artists = now_playing.artists.join(", ");
        let metadata = MediaMetadata {
            title: Some(now_playing.title.as_str()),
            artist: Some(artists.as_str()).filter(|names| !names.is_empty()),
            album: now_playing.album.as_deref(),
            cover_url: now_playing.cover_url.as_deref(),
            duration: now_playing.duration_ms.map(Duration::from_millis),
        };
        self.controls.set_metadata(metadata).map_err(media_error)?;
        self.publish_status()
    }

    pub fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        self.status = status;
        self.publish_status()
    }

    pub fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }

    pub fn set_live_stream(&mut self, _is_live: bool) -> Result<()> {
        Ok(())
    }

    fn publish_status(&mut self) -> Result<()> {
        self.controls
            .set_playback(playback_of(self.status))
            .map_err(media_error)
    }
}

fn playback_of(status: PlaybackStatus) -> MediaPlayback {
    let progress = |position_ms: u64| Some(MediaPosition(Duration::from_millis(position_ms)));
    match status {
        PlaybackStatus::Playing { position_ms } => MediaPlayback::Playing {
            progress: progress(position_ms),
        },
        PlaybackStatus::Paused { position_ms } => MediaPlayback::Paused {
            progress: progress(position_ms),
        },
        PlaybackStatus::Stopped => MediaPlayback::Stopped,
    }
}

fn media_event_from(event: SystemEvent) -> Option<MediaControlEvent> {
    match event {
        SystemEvent::Play => Some(MediaControlEvent::Play),
        SystemEvent::Pause => Some(MediaControlEvent::Pause),
        SystemEvent::Toggle => Some(MediaControlEvent::Toggle),
        SystemEvent::Next => Some(MediaControlEvent::Next),
        SystemEvent::Previous => Some(MediaControlEvent::Previous),
        SystemEvent::Stop => Some(MediaControlEvent::Stop),
        SystemEvent::Seek(direction) => Some(MediaControlEvent::SeekBy {
            offset_ms: signed_offset(direction, SEEK_STEP_MS),
        }),
        SystemEvent::SeekBy(direction, amount) => Some(MediaControlEvent::SeekBy {
            offset_ms: signed_offset(
                direction,
                i64::try_from(amount.as_millis()).unwrap_or(i64::MAX),
            ),
        }),
        SystemEvent::SetPosition(MediaPosition(position)) => Some(MediaControlEvent::SeekTo {
            position_ms: u64::try_from(position.as_millis()).unwrap_or(u64::MAX),
        }),
        SystemEvent::SetVolume(volume) => Some(MediaControlEvent::SetVolume(volume)),
        SystemEvent::Raise => Some(MediaControlEvent::Raise),
        SystemEvent::Quit => Some(MediaControlEvent::Quit),
        SystemEvent::OpenUri(_) => None,
    }
}

fn signed_offset(direction: SeekDirection, amount_ms: i64) -> i64 {
    match direction {
        SeekDirection::Forward => amount_ms,
        SeekDirection::Backward => amount_ms.saturating_neg(),
    }
}

fn media_error(error: souvlaki::Error) -> PlatformError {
    PlatformError::MediaControls(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_events_map_to_player_events() {
        assert_eq!(
            media_event_from(SystemEvent::Toggle),
            Some(MediaControlEvent::Toggle)
        );
        assert_eq!(
            media_event_from(SystemEvent::SetPosition(MediaPosition(
                Duration::from_millis(61_500)
            ))),
            Some(MediaControlEvent::SeekTo {
                position_ms: 61_500
            })
        );
        assert_eq!(
            media_event_from(SystemEvent::Seek(SeekDirection::Backward)),
            Some(MediaControlEvent::SeekBy { offset_ms: -10_000 })
        );
        assert_eq!(
            media_event_from(SystemEvent::SeekBy(
                SeekDirection::Forward,
                Duration::from_secs(5)
            )),
            Some(MediaControlEvent::SeekBy { offset_ms: 5_000 })
        );
        assert_eq!(media_event_from(SystemEvent::OpenUri("x".to_owned())), None);
    }

    #[test]
    fn statuses_carry_their_position() {
        assert_eq!(
            playback_of(PlaybackStatus::Paused { position_ms: 1_200 }),
            MediaPlayback::Paused {
                progress: Some(MediaPosition(Duration::from_millis(1_200)))
            }
        );
        assert_eq!(playback_of(PlaybackStatus::Stopped), MediaPlayback::Stopped);
    }
}
