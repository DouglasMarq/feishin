use std::sync::Arc;

use feishin_storage::settings::{Settings, UiSettings};
use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedSender;

use crate::connect::Connector;
use crate::controller::Command;
use crate::covers::CoverProvider;
use crate::session::Session;
use crate::store::Store;

#[allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]
mod generated {
    slint::include_modules!();
}

pub use generated::*;

pub mod album_detail;
pub mod album_grid;
pub mod albums;
pub mod artist_detail;
pub mod artists;
pub mod favorites;
pub mod genres;
pub mod home;
pub mod login;
pub mod nav;
pub mod now_playing;
pub mod page_cache;
pub mod player;
pub mod playlist_detail;
pub mod playlist_picker;
pub mod playlists;
pub mod queue;
pub mod search;
pub mod settings;
pub mod tracks;

#[derive(Clone)]
pub struct Context {
    pub rt: tokio::runtime::Handle,
    pub commands: UnboundedSender<Command>,
    pub session: Session,
    pub store: Arc<dyn Store>,
    pub connector: Arc<dyn Connector>,
    pub covers: Arc<dyn CoverProvider>,
}

impl Context {
    pub fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::warn!("controller is gone; command dropped");
        }
    }
}

pub fn on_ui(window: &slint::Weak<AppWindow>, update: impl FnOnce(&AppWindow) + Send + 'static) {
    let window = window.clone();
    let queued = slint::invoke_from_event_loop(move || {
        if let Some(window) = window.upgrade() {
            update(&window);
        }
    });
    if let Err(error) = queued {
        tracing::debug!("UI event loop is gone: {error}");
    }
}

pub fn install(window: &AppWindow, ctx: &Context, settings: &Settings) {
    let picker = playlist_picker::install(window, ctx);
    tracks::install(window, ctx, &settings.ui, &picker);
    queue::install(window, ctx);
    player::install(window, ctx);
    login::install(window, ctx);
    nav::install(window);
    home::install(window, ctx);
    albums::install(window, ctx);
    album_detail::install(window, ctx);
    artists::install(window, ctx);
    artist_detail::install(window, ctx);
    genres::install(window, ctx);
    playlists::install(window, ctx);
    playlist_detail::install(window, ctx);
    favorites::install(window, ctx);
    search::install(window, ctx);
    settings::install(window, ctx);
    now_playing::install(window, ctx);
    window
        .global::<AppState>()
        .set_queue_open(settings.ui.queue_panel_open);
}

pub fn ui_settings(window: &AppWindow, previous: &UiSettings) -> UiSettings {
    UiSettings {
        track_columns: tracks::columns(window),
        queue_panel_open: window.global::<AppState>().get_queue_open(),
        ..previous.clone()
    }
}

pub fn show_status(window: &AppWindow, message: &str) {
    window.global::<AppState>().set_status(message.into());
}

pub fn format_duration(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1000;
    let (hours, minutes, seconds) = (
        total_seconds / 3600,
        total_seconds / 60 % 60,
        total_seconds % 60,
    );
    match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    }
}

pub fn clamp_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_format_like_feishin() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(61_000), "1:01");
        assert_eq!(format_duration(3_723_000), "1:02:03");
    }
}
