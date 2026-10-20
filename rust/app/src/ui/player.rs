use feishin_core::queue::{QueueTrack, RepeatMode};
use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, Image};

use super::images::{CoverSource, to_image};
use super::{
    AppWindow, Context, PlayerState, RepeatState, clamp_i32, on_ui, queue, radio, show_status,
    tracks,
};
use crate::controller::{Command, MAX_VOLUME, UiSink, UiUpdate};

const COVER_SIZE: u32 = 128;

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<PlayerState>();
    let bind = |command: fn() -> Command| {
        let context = context.clone();
        move || context.send(command())
    };
    state.on_toggle_pause(bind(|| Command::TogglePause));
    state.on_next(bind(|| Command::Next));
    state.on_previous(bind(|| Command::Previous));
    state.on_toggle_mute(bind(|| Command::ToggleMute));
    state.on_toggle_shuffle(bind(|| Command::ToggleShuffle));
    state.on_cycle_repeat(bind(|| Command::CycleRepeat));
    let seek_context = context.clone();
    state.on_seek(move |position_ms| {
        seek_context.send(Command::Seek(u64::try_from(position_ms).unwrap_or(0)));
    });
    let volume_context = context.clone();
    state.on_set_volume(move |volume| {
        volume_context.send(Command::SetVolume(
            u8::try_from(volume.clamp(0, i32::from(MAX_VOLUME))).unwrap_or_default(),
        ));
    });
}

pub struct SlintSink {
    window: slint::Weak<AppWindow>,
    context: Context,
}

impl SlintSink {
    pub fn new(window: slint::Weak<AppWindow>, context: Context) -> Self {
        Self { window, context }
    }

    fn load_cover(&self, track: &QueueTrack) {
        let Some(source) = CoverSource::of_track(track, self.context.session.get()) else {
            return;
        };
        let song_id = track.song_id.to_string();
        let covers = self.context.covers.clone();
        let window = self.window.clone();
        self.context.runtime.spawn(async move {
            if let Some(cover) = source.fetch(covers.as_ref(), COVER_SIZE).await {
                on_ui(&window, move |window| show_cover(window, &song_id, &cover));
            }
        });
    }
}

fn show_cover(window: &AppWindow, song_id: &str, cover: &DecodedImage) {
    let state = window.global::<PlayerState>();
    if state.get_song_id() != song_id {
        return;
    }
    if let Some(image) = to_image(cover) {
        state.set_cover(image);
        state.set_has_cover(true);
    }
}

impl UiSink for SlintSink {
    fn send(&self, update: UiUpdate) {
        let cover_for = match &update {
            UiUpdate::NowPlaying(Some(track)) => Some(track.clone()),
            _ => None,
        };
        on_ui(&self.window, move |window| apply(window, update));
        if let Some(track) = cover_for {
            self.load_cover(&track);
        }
    }
}

fn show_song(window: &AppWindow, track: &QueueTrack) {
    let state = window.global::<PlayerState>();
    let song_id = track.song_id.as_str();
    if state.get_song_id() != song_id {
        state.set_has_cover(false);
        state.set_cover(Image::default());
    }
    state.set_has_song(true);
    state.set_song_id(song_id.into());
    state.set_title((*track.title).into());
    state.set_artists(track.artist_names().into());
    state.set_album(track.album.as_deref().unwrap_or_default().into());
    tracks::set_current(window, Some(song_id));
}

fn clear_song(window: &AppWindow) {
    let state = window.global::<PlayerState>();
    state.set_has_song(false);
    state.set_has_cover(false);
    state.set_cover(Image::default());
    state.set_song_id("".into());
    state.set_title("".into());
    state.set_artists("".into());
    state.set_album("".into());
    tracks::set_current(window, None);
}

fn repeat_state(repeat: RepeatMode) -> RepeatState {
    match repeat {
        RepeatMode::None => RepeatState::Off,
        RepeatMode::All => RepeatState::All,
        RepeatMode::One => RepeatState::One,
    }
}

fn raise(window: &AppWindow) {
    if let Err(error) = window.show() {
        tracing::debug!("raise: {error}");
    }
}

fn quit() {
    if let Err(error) = slint::quit_event_loop() {
        tracing::debug!("quit: {error}");
    }
}

fn apply(window: &AppWindow, update: UiUpdate) {
    let state = window.global::<PlayerState>();
    match update {
        UiUpdate::NowPlaying(Some(track)) => show_song(window, &track),
        UiUpdate::NowPlaying(None) => clear_song(window),
        UiUpdate::Playback {
            playing,
            position_ms,
            duration_ms,
        } => {
            state.set_playing(playing);
            state.set_position_ms(clamp_i32(position_ms));
            state.set_duration_ms(clamp_i32(duration_ms));
        }
        UiUpdate::Volume { volume, muted } => {
            state.set_volume(i32::from(volume));
            state.set_muted(muted);
        }
        UiUpdate::Modes { shuffle, repeat } => {
            state.set_shuffle(shuffle);
            state.set_repeat(repeat_state(repeat));
        }
        UiUpdate::Queue(view) => queue::set_view(window, view),
        UiUpdate::Radio(now_playing) => radio::show_now_playing(window, now_playing.as_ref()),
        UiUpdate::Error(message) => show_status(window, &message),
        UiUpdate::Raise => raise(window),
        UiUpdate::Quit => quit(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_each_repeat_mode_the_player_bar_shows_the_matching_state() {
        assert_eq!(repeat_state(RepeatMode::None), RepeatState::Off);
        assert_eq!(repeat_state(RepeatMode::All), RepeatState::All);
        assert_eq!(repeat_state(RepeatMode::One), RepeatState::One);
    }
}
