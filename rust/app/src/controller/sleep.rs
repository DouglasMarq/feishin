use feishin_core::queue::Advance;

use super::Controller;
use crate::sleep_timer::SleepMode;

impl Controller {
    pub(super) fn set_sleep_timer(&mut self, mode: Option<SleepMode>) {
        match mode {
            Some(mode) => self.sleep_timer.start(mode, &self.deps.commands),
            None => self.sleep_timer.cancel(),
        }
        self.deps.ui.sleep_timer(self.sleep_timer.status());
        self.refresh_preload();
    }

    pub(super) fn sleep_tick(&mut self, elapsed_ms: u64) {
        if self.sleep_timer.tick(elapsed_ms, self.playing) && self.playing {
            self.toggle_pause();
        }
        self.deps.ui.sleep_timer(self.sleep_timer.status());
    }

    pub(super) fn sleep_at_track_end(&mut self) {
        self.sleep_timer.cancel();
        self.deps.ui.sleep_timer(None);
        self.queue.next(Advance::Auto);
        self.stop();
        self.publish_now_playing();
        self.publish_queue_current();
        self.schedule_queue_sync();
    }
}
