use feishin_core::model::Song;
use feishin_core::queue::{RepeatMode, Uid};

use super::{Controller, UiUpdate};

fn next_repeat_mode(mode: RepeatMode) -> RepeatMode {
    match mode {
        RepeatMode::None => RepeatMode::All,
        RepeatMode::All => RepeatMode::One,
        RepeatMode::One => RepeatMode::None,
    }
}

impl Controller {
    pub(super) fn toggle_shuffle(&mut self) {
        let shuffled = !self.queue.shuffle();
        self.queue.set_shuffle(shuffled);
        self.queue_changed();
    }

    pub(super) fn cycle_repeat(&mut self) {
        self.queue.set_repeat(next_repeat_mode(self.queue.repeat()));
        self.publish_modes();
        self.refresh_preload();
    }

    pub(super) fn play_uid(&mut self, uid: Uid) {
        if self.queue.play_uid(uid).is_some() {
            self.load_current(0);
        }
    }

    pub(super) fn play_songs(&mut self, songs: Vec<Song>, song_id: Option<String>) {
        self.play_request += 1;
        self.list_streams.cancel_all();
        self.queue.play_now(songs, song_id.as_deref());
        self.publish_queue();
        self.load_current(0);
    }

    pub(super) fn add_next(&mut self, songs: Vec<Song>) {
        self.queue.add_next(songs);
        self.queue_changed();
    }

    pub(super) fn add_last(&mut self, songs: Vec<Song>) {
        self.queue.add_last(songs);
        self.queue_changed();
    }

    pub(super) fn move_items(&mut self, uids: &[Uid], before: Option<Uid>) {
        self.queue.move_items(uids, before);
        self.queue_changed();
    }

    pub(super) fn remove(&mut self, uids: &[Uid]) {
        let loaded_removed = self.loaded.is_some_and(|uid| uids.contains(&uid));
        self.queue.remove(uids);
        match (self.queue.is_empty(), loaded_removed) {
            (true, _) => self.stop_with_empty_queue(),
            (false, true) => self.move_past_removed_track(),
            (false, false) => {}
        }
        self.queue_changed();
    }

    fn move_past_removed_track(&mut self) {
        if self.playing {
            self.load_current(0);
            return;
        }
        self.stop();
        self.publish_now_playing();
    }

    fn stop_with_empty_queue(&mut self) {
        self.stop();
        self.deps.ui.send(UiUpdate::NowPlaying(None));
    }

    pub(super) fn clear_queue(&mut self) {
        self.list_streams.cancel_all();
        self.queue.clear();
        self.stop_with_empty_queue();
        self.queue_changed();
    }

    pub(super) fn queue_changed(&mut self) {
        self.publish_queue();
        self.publish_modes();
        self.refresh_preload();
        self.schedule_queue_sync();
    }
}
