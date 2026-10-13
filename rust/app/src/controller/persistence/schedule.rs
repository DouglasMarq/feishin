use std::time::Duration;

pub(super) const QUEUE_SAVE_DELAY: Duration = Duration::from_secs(10);
pub(super) const PLAYBACK_SAVE_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Queue,
    Playback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveTimer {
    pub(super) kind: SaveKind,
    pub(super) generation: u64,
}

impl SaveTimer {
    pub(super) fn delay(self) -> Duration {
        match self.kind {
            SaveKind::Queue => QUEUE_SAVE_DELAY,
            SaveKind::Playback => PLAYBACK_SAVE_DELAY,
        }
    }
}

#[derive(Debug)]
pub(super) struct SaveSchedule {
    saved_revision: Option<u64>,
    armed_revision: Option<u64>,
    generation: u64,
}

impl Default for SaveSchedule {
    fn default() -> Self {
        Self {
            saved_revision: Some(0),
            armed_revision: None,
            generation: 0,
        }
    }
}

impl SaveSchedule {
    pub(super) fn plan(&mut self, revision: u64) -> Option<SaveTimer> {
        match (
            self.saved_revision == Some(revision),
            self.armed_revision == Some(revision),
        ) {
            (true, _) => Some(self.arm(SaveKind::Playback)),
            (false, true) => None,
            (false, false) => {
                self.armed_revision = Some(revision);
                Some(self.arm(SaveKind::Queue))
            }
        }
    }

    pub(super) fn is_due(&self, timer: SaveTimer, revision: u64) -> bool {
        timer.generation == self.generation
            && (timer.kind == SaveKind::Queue || self.saved_revision == Some(revision))
    }

    pub(super) fn has_unsaved_changes(&self, revision: u64) -> bool {
        self.saved_revision != Some(revision)
    }

    pub(super) fn mark_saved(&mut self, revision: u64) {
        self.saved_revision = Some(revision);
        self.armed_revision = None;
        self.generation += 1;
    }

    pub(super) fn mark_failed(&mut self, revision: u64) {
        if self.saved_revision == Some(revision) {
            self.saved_revision = None;
        }
    }

    fn arm(&mut self, kind: SaveKind) -> SaveTimer {
        self.generation += 1;
        SaveTimer {
            kind,
            generation: self.generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_changes_only_save_playback() {
        let mut schedule = SaveSchedule::default();
        let timer = schedule.plan(0).unwrap();
        assert_eq!(timer.kind, SaveKind::Playback);
        assert!(schedule.is_due(timer, 0));
        let newer = schedule.plan(0).unwrap();
        assert!(!schedule.is_due(timer, 0));
        assert!(schedule.is_due(newer, 0));
    }

    #[test]
    fn structural_changes_rearm_the_queue_save() {
        let mut schedule = SaveSchedule::default();
        let first = schedule.plan(1).unwrap();
        assert_eq!(first.kind, SaveKind::Queue);
        assert_eq!(schedule.plan(1), None);
        let second = schedule.plan(2).unwrap();
        assert!(!schedule.is_due(first, 2));
        assert!(schedule.is_due(second, 2));
        assert!(schedule.has_unsaved_changes(2));
        schedule.mark_saved(2);
        assert!(!schedule.has_unsaved_changes(2));
        assert!(!schedule.is_due(second, 2));
        assert_eq!(
            schedule.plan(2).map(|timer| timer.kind),
            Some(SaveKind::Playback)
        );
    }

    #[test]
    fn a_pending_playback_save_is_dropped_once_the_queue_changes() {
        let mut schedule = SaveSchedule::default();
        let playback = schedule.plan(0).unwrap();
        let queue = schedule.plan(1).unwrap();
        assert!(!schedule.is_due(playback, 1));
        assert!(schedule.is_due(queue, 1));
        assert_eq!(queue.delay(), QUEUE_SAVE_DELAY);
        assert_eq!(playback.delay(), PLAYBACK_SAVE_DELAY);
    }

    #[test]
    fn a_failed_queue_save_is_retried_on_the_next_change_or_at_shutdown() {
        let mut schedule = SaveSchedule::default();
        schedule.mark_saved(3);
        schedule.mark_failed(2);
        assert!(!schedule.has_unsaved_changes(3));
        schedule.mark_failed(3);
        assert!(schedule.has_unsaved_changes(3));
        assert_eq!(
            schedule.plan(3).map(|timer| timer.kind),
            Some(SaveKind::Queue)
        );
        assert_eq!(schedule.plan(3), None);
    }
}
