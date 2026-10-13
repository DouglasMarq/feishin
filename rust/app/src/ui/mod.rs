//! Slint views and the glue between them and the controller. Each view module installs its own callbacks.

use std::sync::Arc;

use feishin_storage::settings::{Settings, UiSettings};
use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedSender;

use crate::connect::Connector;
use crate::controller::Command;
use crate::covers::CoverProvider;
use crate::session::Session;
use crate::store::Store;

slint::include_modules!();

pub mod login;
pub mod player;
pub mod queue;
pub mod tracks;

/// Everything the views need to reach the I/O side. Cheap to clone and `Send`.
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

/// Runs `f` on the UI thread if the window still exists.
pub fn on_ui(window: &slint::Weak<AppWindow>, f: impl FnOnce(&AppWindow) + Send + 'static) {
    let window = window.clone();
    let queued = slint::invoke_from_event_loop(move || {
        if let Some(window) = window.upgrade() {
            f(&window);
        }
    });
    if let Err(e) = queued {
        tracing::debug!("UI event loop is gone: {e}");
    }
}

pub fn install(window: &AppWindow, ctx: &Context, settings: &Settings) {
    tracks::install(window, ctx, &settings.ui);
    queue::install(window, ctx);
    player::install(window, ctx);
    login::install(window, ctx);
    window
        .global::<AppState>()
        .set_queue_open(settings.ui.queue_panel_open);
}

/// UI settings to persist, read back from the window.
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

pub fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
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
