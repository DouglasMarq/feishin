use std::sync::Arc;

use feishin_core::model::SortOrder;
use feishin_remote::ServerKey;
use feishin_storage::settings::{Settings, UiSettings};
use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedSender;

use crate::connect::Connector;
use crate::controller::Command;
use crate::covers::CoverProvider;
use crate::remote::Remote;
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
pub mod appearance;
pub mod artist_detail;
pub mod artists;
pub mod cover_lru;
pub mod devices;
pub mod discover;
pub mod favorites;
pub mod folders;
pub mod genres;
pub mod home;
pub mod images;
pub mod library_scan;
pub mod login;
pub mod nav;
pub mod now_playing;
pub mod page_backoff;
pub mod page_cache;
pub mod player;
pub mod playlist_detail;
pub mod playlist_picker;
pub mod playlists;
pub mod queue;
pub mod radio;
pub mod search;
pub mod settings;
pub mod sharing;
pub mod tracks;

#[cfg(test)]
mod sharing_view_tests;
#[cfg(test)]
pub mod test_window;

pub const MAX_RATING: u8 = 5;

#[derive(Clone)]
pub struct Context {
    pub runtime: tokio::runtime::Handle,
    pub commands: UnboundedSender<Command>,
    pub session: Session,
    pub store: Arc<dyn Store>,
    pub connector: Arc<dyn Connector>,
    pub covers: Arc<dyn CoverProvider>,
    pub remote: Remote,
}

impl Context {
    pub fn send(&self, command: Command) {
        let routed = match self.remote.target.is_remote() {
            true => {
                let local_server = self
                    .session
                    .get()
                    .map(|server| ServerKey::of(server.config()));
                self.remote.target.route(command, local_server.as_ref())
            }
            false => Some(command),
        };
        let Some(command) = routed else {
            return;
        };
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

pub fn install(window: &AppWindow, context: &Context, settings: &Settings) {
    let picker = playlist_picker::install(window, context);
    tracks::install(window, context, &settings.ui, &picker);
    queue::install(window, context);
    player::install(window, context);
    login::install(window, context);
    nav::install(window);
    library_scan::install(window, context);
    home::install(window, context);
    albums::install(window, context);
    album_detail::install(window, context);
    artists::install(window, context);
    artist_detail::install(window, context);
    genres::install(window, context);
    playlists::install(window, context);
    playlist_detail::install(window, context, &picker);
    favorites::install(window, context);
    folders::install(window, context);
    search::install(window, context);
    settings::install(window, context);
    sharing::install(window, context);
    now_playing::install(window, context);
    appearance::install(window, context, &settings.appearance);
    radio::install(window, context);
    devices::install(window, context);
    window
        .global::<AppState>()
        .set_is_queue_open(settings.ui.is_queue_panel_open);
}

pub fn ui_settings(window: &AppWindow, previous: &UiSettings) -> UiSettings {
    UiSettings {
        track_columns: tracks::columns(window),
        is_queue_panel_open: window.global::<AppState>().get_is_queue_open(),
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

pub fn sort_order(is_descending: bool) -> SortOrder {
    match is_descending {
        true => SortOrder::Desc,
        false => SortOrder::Asc,
    }
}

pub fn time_based_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

pub fn slint_index(index: usize) -> i32 {
    i32::try_from(index).unwrap_or_default()
}

pub fn rating_stars(rating: i32) -> u8 {
    u8::try_from(rating.clamp(0, i32::from(MAX_RATING))).unwrap_or_default()
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

    #[test]
    fn given_a_rating_outside_the_star_range_it_is_clamped_to_zero_or_five_stars() {
        assert_eq!(rating_stars(-1), 0);
        assert_eq!(rating_stars(3), 3);
        assert_eq!(rating_stars(9), 5);
    }

    #[test]
    fn given_a_time_based_id_it_is_a_hexadecimal_timestamp() {
        let id = time_based_id();
        assert!(u128::from_str_radix(&id, 16).is_ok_and(|nanos| nanos > 0));
    }

    #[test]
    fn given_an_index_beyond_slint_range_it_falls_back_to_the_first_row() {
        assert_eq!(slint_index(7), 7);
        assert_eq!(slint_index(usize::MAX), 0);
    }
}
