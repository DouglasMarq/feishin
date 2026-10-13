use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::model::Song;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::lyrics_cache::{CachedLyrics, LyricsCache};
use super::song_lyrics::{SongLyrics, active_line};
use super::up_next::UpNextModel;
use crate::ui::{AppWindow, LyricRow, LyricsStatus, NowPlayingState, PlayerState, clamp_i32};

const LYRICS_CACHE_SIZE: usize = 32;
const NO_INDEX: i32 = -1;

pub(super) struct View {
    cache: RefCell<Option<LyricsCache>>,
    line_times_ms: RefCell<Vec<u64>>,
    pub(super) similar: RefCell<Vec<Song>>,
    up_next: Rc<UpNextModel>,
}

impl View {
    pub(super) fn new(up_next: Rc<UpNextModel>) -> Self {
        Self {
            cache: RefCell::default(),
            line_times_ms: RefCell::default(),
            similar: RefCell::default(),
            up_next,
        }
    }

    pub(super) fn cached_lyrics(&self, song_id: &str) -> Option<CachedLyrics> {
        self.cache.borrow_mut().as_mut()?.get(song_id)
    }

    pub(super) fn remember(&self, song_id: String, lyrics: CachedLyrics) {
        self.cache
            .borrow_mut()
            .get_or_insert_with(|| LyricsCache::new(LYRICS_CACHE_SIZE))
            .insert(song_id, lyrics);
    }

    pub(super) fn highlight_active_line(&self, window: &AppWindow) {
        let position_ms = u64::try_from(window.global::<PlayerState>().get_position_ms());
        let line = active_line(&self.line_times_ms.borrow(), position_ms.unwrap_or(0));
        let line = line.map_or(NO_INDEX, |line| clamp_i32(line as u64));
        let state = window.global::<NowPlayingState>();
        if state.get_active_line() != line {
            state.set_active_line(line);
        }
    }

    pub(super) fn show_lyrics(&self, window: &AppWindow, lyrics: Option<&SongLyrics>) {
        let Some(lyrics) = lyrics else {
            self.show_missing_lyrics(window);
            return;
        };
        let state = window.global::<NowPlayingState>();
        let rows: Vec<LyricRow> = lyrics
            .lines
            .iter()
            .map(|line| LyricRow {
                text: line.text.as_str().into(),
                time_ms: line.time_ms.map_or(NO_INDEX, clamp_i32),
            })
            .collect();
        *self.line_times_ms.borrow_mut() = lyrics.line_times_ms();
        state.set_lyrics(ModelRc::new(VecModel::from(rows)));
        state.set_lyrics_synced(lyrics.synced);
        state.set_lyrics_source(lyrics.source.into());
        state.set_lyrics_status(LyricsStatus::Found);
        state.set_active_line(NO_INDEX);
        self.highlight_active_line(window);
    }

    fn show_missing_lyrics(&self, window: &AppWindow) {
        let state = window.global::<NowPlayingState>();
        *self.line_times_ms.borrow_mut() = Vec::new();
        state.set_lyrics(ModelRc::default());
        state.set_lyrics_status(LyricsStatus::Missing);
    }

    pub(super) fn refresh_up_next(&self, window: &AppWindow) {
        let offset = self.up_next.refresh();
        window
            .global::<NowPlayingState>()
            .set_up_next_offset(offset.map_or(NO_INDEX, |offset| clamp_i32(offset as u64)));
    }

    pub(super) fn clear(&self, window: &AppWindow, song_id: &str) {
        let state = window.global::<NowPlayingState>();
        state.set_song_id(song_id.into());
        state.set_loaded(false);
        state.set_artists(ModelRc::default());
        state.set_album_id(SharedString::default());
        state.set_has_cover(false);
        state.set_cover(slint::Image::default());
        state.set_lyrics(ModelRc::default());
        state.set_lyrics_synced(false);
        state.set_lyrics_source(SharedString::default());
        state.set_lyrics_status(LyricsStatus::Idle);
        state.set_active_line(NO_INDEX);
        state.set_similar(ModelRc::default());
        state.set_similar_loading(false);
        self.line_times_ms.borrow_mut().clear();
        self.similar.borrow_mut().clear();
    }
}

thread_local! {
    static VIEW: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
}

pub(super) fn register_view(view: &Rc<View>) {
    VIEW.with(|slot| *slot.borrow_mut() = Some(Rc::clone(view)));
}

pub(super) fn with_view(action: impl FnOnce(&View)) {
    if let Some(view) = VIEW.with(|slot| slot.borrow().clone()) {
        action(&view);
    }
}
