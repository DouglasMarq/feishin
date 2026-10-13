use std::time::Instant;

use mpris_server::zbus::{self, fdo};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus as MprisStatus, PlayerInterface, Property,
    Time, TrackId, Volume,
};

use super::MprisPlayer;
use super::conversions::{not_supported, time_from_ms, volume_from_mpris};
use crate::MediaControlEvent;

const FIXED_PLAYBACK_RATE: PlaybackRate = 1.0;

fn toggled_status(status: MprisStatus) -> MprisStatus {
    match status {
        MprisStatus::Playing => MprisStatus::Paused,
        MprisStatus::Paused | MprisStatus::Stopped => MprisStatus::Playing,
    }
}

impl PlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        self.core
            .transport(MediaControlEvent::Next, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.core
            .transport(MediaControlEvent::Previous, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.core
            .transport(MediaControlEvent::Pause, |_| MprisStatus::Paused);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.core
            .transport(MediaControlEvent::Toggle, toggled_status);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.core.emit(MediaControlEvent::Stop);
        self.core.update_status(|_| MprisStatus::Paused);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.core
            .transport(MediaControlEvent::Play, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        self.core.emit(MediaControlEvent::SeekBy {
            offset_ms: offset.as_millis(),
        });
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        if let Ok(position_ms) = u64::try_from(position.as_millis()) {
            self.core.emit(MediaControlEvent::SeekTo { position_ms });
        }
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(not_supported("OpenUri"))
    }

    async fn playback_status(&self) -> fdo::Result<MprisStatus> {
        Ok(self.core.snapshot().status)
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        Err(not_supported("LoopStatus").into())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(FIXED_PLAYBACK_RATE)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Err(not_supported("Rate").into())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Err(not_supported("Shuffle").into())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.core.snapshot().metadata.clone())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.core.snapshot().volume)
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        let volume = volume_from_mpris(volume);
        self.core.snapshot().volume = volume;
        self.core.notify(vec![Property::Volume(volume)]);
        self.core.emit(MediaControlEvent::SetVolume(volume));
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(time_from_ms(
            self.core.snapshot().position_ms(Instant::now()),
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(FIXED_PLAYBACK_RATE)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(FIXED_PLAYBACK_RATE)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
