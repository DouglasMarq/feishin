use feishin_core::model::Song;
use feishin_core::queue::RepeatMode;
use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};

use super::{AppWindow, Context, PlayerState, clamp_i32, on_ui, queue, show_status, tracks};
use crate::controller::{Command, UiSink, UiUpdate, artist_names};

const COVER_SIZE: u32 = 128;

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<PlayerState>();
    let bind = |command: fn() -> Command| {
        let ctx = ctx.clone();
        move || ctx.send(command())
    };
    state.on_toggle_pause(bind(|| Command::TogglePause));
    state.on_next(bind(|| Command::Next));
    state.on_previous(bind(|| Command::Previous));
    state.on_toggle_mute(bind(|| Command::ToggleMute));
    state.on_toggle_shuffle(bind(|| Command::ToggleShuffle));
    state.on_cycle_repeat(bind(|| Command::CycleRepeat));
    let seek_ctx = ctx.clone();
    state.on_seek(move |ms| seek_ctx.send(Command::Seek(u64::try_from(ms).unwrap_or(0))));
    let volume_ctx = ctx.clone();
    state.on_set_volume(move |volume| {
        volume_ctx.send(Command::SetVolume(volume.clamp(0, 100) as u8));
    });
}

/// Delivers controller updates to the window from any thread.
pub struct SlintSink {
    window: slint::Weak<AppWindow>,
    ctx: Context,
}

impl SlintSink {
    pub fn new(window: slint::Weak<AppWindow>, ctx: Context) -> Self {
        Self { window, ctx }
    }

    fn load_cover(&self, song: &Song) {
        let (Some(cover_id), Some(server)) = (song.cover_art_id.clone(), self.ctx.session.get())
        else {
            return;
        };
        let song_id = song.id.clone();
        let covers = self.ctx.covers.clone();
        let window = self.window.clone();
        self.ctx.rt.spawn(async move {
            match covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await {
                Ok(cover) => on_ui(&window, move |window| {
                    let state = window.global::<PlayerState>();
                    if state.get_song_id() == song_id.as_str()
                        && let Some(image) = to_image(&cover)
                    {
                        state.set_cover(image);
                        state.set_has_cover(true);
                    }
                }),
                Err(e) => tracing::debug!("cover {cover_id}: {e:#}"),
            }
        });
    }
}

pub fn to_image(cover: &DecodedImage) -> Option<Image> {
    let expected = cover.width as usize * cover.height as usize * 4;
    if cover.rgba.len() != expected || expected == 0 {
        return None;
    }
    let buffer =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&cover.rgba, cover.width, cover.height);
    Some(Image::from_rgba8(buffer))
}

impl UiSink for SlintSink {
    fn send(&self, update: UiUpdate) {
        let cover_for = match &update {
            UiUpdate::NowPlaying(Some(song)) => Some(song.clone()),
            _ => None,
        };
        on_ui(&self.window, move |window| apply(window, update));
        if let Some(song) = cover_for {
            self.load_cover(&song);
        }
    }
}

fn apply(window: &AppWindow, update: UiUpdate) {
    let state = window.global::<PlayerState>();
    match update {
        UiUpdate::NowPlaying(Some(song)) => {
            if state.get_song_id() != song.id.as_str() {
                state.set_has_cover(false);
                state.set_cover(Image::default());
            }
            state.set_has_song(true);
            state.set_song_id(song.id.as_str().into());
            state.set_title(song.title.as_str().into());
            state.set_artists(artist_names(&song).into());
            state.set_album(song.album.unwrap_or_default().into());
            tracks::set_current(window, Some(&song.id));
        }
        UiUpdate::NowPlaying(None) => {
            state.set_has_song(false);
            state.set_has_cover(false);
            state.set_cover(Image::default());
            state.set_song_id("".into());
            state.set_title("".into());
            state.set_artists("".into());
            state.set_album("".into());
            tracks::set_current(window, None);
        }
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
            state.set_repeat(match repeat {
                RepeatMode::None => 0,
                RepeatMode::All => 1,
                RepeatMode::One => 2,
            });
        }
        UiUpdate::Queue { entries, current } => queue::set_entries(window, entries, current),
        UiUpdate::QueueCurrent(current) => queue::set_current(window, current),
        UiUpdate::Error(message) => show_status(window, &message),
        UiUpdate::Raise => {
            if let Err(e) = window.show() {
                tracing::debug!("raise: {e}");
            }
        }
        UiUpdate::Quit => {
            if let Err(e) = slint::quit_event_loop() {
                tracing::debug!("quit: {e}");
            }
        }
    }
}
