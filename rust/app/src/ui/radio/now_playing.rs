use slint::ComponentHandle;

use crate::controller::RadioNowPlaying;
use crate::ui::{AppWindow, RadioState};

pub fn show_now_playing(window: &AppWindow, now_playing: Option<&RadioNowPlaying>) {
    let state = window.global::<RadioState>();
    state.set_is_active(now_playing.is_some());
    let Some(now_playing) = now_playing else {
        state.set_playing_id("".into());
        state.set_station_name("".into());
        state.set_stream_title("".into());
        return;
    };
    state.set_playing_id(now_playing.station_id.as_str().into());
    state.set_station_name(now_playing.station_name.as_str().into());
    state.set_stream_title(
        now_playing
            .stream_title
            .as_deref()
            .unwrap_or_default()
            .into(),
    );
}
