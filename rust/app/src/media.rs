use anyhow::Result;
use feishin_platform::{MediaControlEvent, MediaControls, NowPlaying, PlaybackStatus};

/// OS media session (MPRIS on Linux through `feishin_platform::MediaControls`).
pub trait MediaSession: Send {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()>;
    fn set_status(&mut self, status: PlaybackStatus) -> Result<()>;
    fn set_volume(&mut self, volume: f64) -> Result<()>;
}

pub struct Mpris(MediaControls);

impl Mpris {
    pub fn new(on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>) -> Result<Self> {
        Ok(Self(MediaControls::new("Feishin", on_event)?))
    }
}

impl MediaSession for Mpris {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        Ok(self.0.set_metadata(now_playing)?)
    }
    fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        Ok(self.0.set_status(status)?)
    }
    fn set_volume(&mut self, volume: f64) -> Result<()> {
        Ok(self.0.set_volume(volume)?)
    }
}

pub struct NoMediaSession;

impl MediaSession for NoMediaSession {
    fn set_metadata(&mut self, _now_playing: &NowPlaying) -> Result<()> {
        Ok(())
    }
    fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        Ok(())
    }
    fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }
}

/// Feeds several sessions at once (MPRIS and Discord); every session is updated even if one fails.
pub struct MediaSessions(pub Vec<Box<dyn MediaSession>>);

impl MediaSessions {
    fn each(&mut self, mut f: impl FnMut(&mut dyn MediaSession) -> Result<()>) -> Result<()> {
        let mut result = Ok(());
        for session in &mut self.0 {
            if let Err(e) = f(session.as_mut()) {
                result = Err(e);
            }
        }
        result
    }
}

impl MediaSession for MediaSessions {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.each(|session| session.set_metadata(now_playing))
    }
    fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        self.each(|session| session.set_status(status))
    }
    fn set_volume(&mut self, volume: f64) -> Result<()> {
        self.each(|session| session.set_volume(volume))
    }
}
