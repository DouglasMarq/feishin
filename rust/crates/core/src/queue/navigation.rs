use super::{Advance, FIRST_POSITION, Queue, QueueItem, RepeatMode, Uid};

enum NextStep {
    MoveTo(usize),
    StopPlayback,
}

impl Queue {
    pub fn next(&mut self, advance: Advance) -> Option<&QueueItem> {
        match self.next_step(advance)? {
            NextStep::MoveTo(position) => {
                self.current_uid = self.uid_at_position(position);
                self.current()
            }
            NextStep::StopPlayback => {
                self.current_uid = self.uid_at_position(FIRST_POSITION);
                None
            }
        }
    }

    pub fn previous(&mut self) -> Option<&QueueItem> {
        let last_position = self.items.len().checked_sub(1)?;
        let previous_position = match self.current_position() {
            None => FIRST_POSITION,
            Some(FIRST_POSITION) if self.repeat == RepeatMode::All => last_position,
            Some(position) => position.saturating_sub(1),
        };
        self.current_uid = self.uid_at_position(previous_position);
        self.current()
    }

    pub fn peek_next(&self) -> Option<&QueueItem> {
        match self.next_step(Advance::Auto)? {
            NextStep::MoveTo(position) => self.item_at_position(position),
            NextStep::StopPlayback => None,
        }
    }

    pub fn play_uid(&mut self, uid: Uid) -> Option<&QueueItem> {
        self.index_of(uid)?;
        self.current_uid = Some(uid);
        self.current()
    }

    fn next_step(&self, advance: Advance) -> Option<NextStep> {
        let item_count = self.items.len();
        let last_position = item_count.checked_sub(1)?;
        let Some(position) = self.current_position() else {
            return Some(NextStep::MoveTo(FIRST_POSITION));
        };
        let following_position = position + 1;
        Some(match (self.repeat, advance) {
            (RepeatMode::One, Advance::Auto) => NextStep::MoveTo(position),
            (RepeatMode::One, Advance::Manual) => {
                NextStep::MoveTo(following_position.min(last_position))
            }
            (RepeatMode::All, _) => NextStep::MoveTo(following_position % item_count),
            (RepeatMode::None, _) if following_position < item_count => {
                NextStep::MoveTo(following_position)
            }
            (RepeatMode::None, _) => NextStep::StopPlayback,
        })
    }
}
