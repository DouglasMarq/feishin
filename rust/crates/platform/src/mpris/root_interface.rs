use mpris_server::RootInterface;
use mpris_server::zbus::{self, fdo};

use super::MprisPlayer;
use super::conversions::not_supported;
use crate::MediaControlEvent;

impl RootInterface for MprisPlayer {
    async fn raise(&self) -> fdo::Result<()> {
        self.core.emit(MediaControlEvent::Raise);
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.core.emit(MediaControlEvent::Quit);
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Err(not_supported("Fullscreen").into())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok(self.core.identity.clone())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(self.core.bus_suffix.clone())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".to_owned()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["audio/mpeg".to_owned(), "application/ogg".to_owned()])
    }
}
