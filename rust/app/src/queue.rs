use std::sync::OnceLock;

use feishin_core::model::Song;
use feishin_core::queue::{Advance, Queue, QueueItem, QueueSnapshot, RepeatMode, Uid};

/// The `feishin_core::queue::Queue` API the controller relies on, so a fallback can stand in
/// while the core crate is still being implemented.
pub trait PlayQueue: Send {
    fn display_order(&self) -> Vec<&QueueItem>;
    fn len(&self) -> usize;
    fn current(&self) -> Option<&QueueItem>;
    fn shuffle(&self) -> bool;
    fn set_shuffle(&mut self, on: bool);
    fn repeat(&self) -> RepeatMode;
    fn set_repeat(&mut self, mode: RepeatMode);
    fn play_now(&mut self, songs: Vec<Song>, start_song_id: Option<&str>) -> Option<&QueueItem>;
    fn next(&mut self, advance: Advance) -> Option<&QueueItem>;
    fn previous(&mut self) -> Option<&QueueItem>;
    fn peek_next(&self) -> Option<&QueueItem>;
    fn play_uid(&mut self, uid: Uid) -> Option<&QueueItem>;
    fn remove(&mut self, uids: &[Uid]);
    fn move_items(&mut self, uids: &[Uid], before_uid: Option<Uid>);
    fn snapshot(&self) -> QueueSnapshot;
}

macro_rules! delegate_play_queue {
    ($ty:ty) => {
        impl PlayQueue for $ty {
            fn display_order(&self) -> Vec<&QueueItem> {
                <$ty>::display_order(self)
            }
            fn len(&self) -> usize {
                <$ty>::len(self)
            }
            fn current(&self) -> Option<&QueueItem> {
                <$ty>::current(self)
            }
            fn shuffle(&self) -> bool {
                <$ty>::shuffle(self)
            }
            fn set_shuffle(&mut self, on: bool) {
                <$ty>::set_shuffle(self, on)
            }
            fn repeat(&self) -> RepeatMode {
                <$ty>::repeat(self)
            }
            fn set_repeat(&mut self, mode: RepeatMode) {
                <$ty>::set_repeat(self, mode)
            }
            fn play_now(
                &mut self,
                songs: Vec<Song>,
                start_song_id: Option<&str>,
            ) -> Option<&QueueItem> {
                <$ty>::play_now(self, songs, start_song_id)
            }
            fn next(&mut self, advance: Advance) -> Option<&QueueItem> {
                <$ty>::next(self, advance)
            }
            fn previous(&mut self) -> Option<&QueueItem> {
                <$ty>::previous(self)
            }
            fn peek_next(&self) -> Option<&QueueItem> {
                <$ty>::peek_next(self)
            }
            fn play_uid(&mut self, uid: Uid) -> Option<&QueueItem> {
                <$ty>::play_uid(self, uid)
            }
            fn remove(&mut self, uids: &[Uid]) {
                <$ty>::remove(self, uids)
            }
            fn move_items(&mut self, uids: &[Uid], before_uid: Option<Uid>) {
                <$ty>::move_items(self, uids, before_uid)
            }
            fn snapshot(&self) -> QueueSnapshot {
                <$ty>::snapshot(self)
            }
        }
    };
}

delegate_play_queue!(Queue);

/// True once `feishin_core::queue::Queue` is implemented (its stub panics with `todo!()`).
fn core_queue_ready() -> bool {
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let ready = std::panic::catch_unwind(|| {
            let mut queue = Queue::with_seed(1);
            queue.play_now(Vec::new(), None);
            queue.len()
        })
        .is_ok();
        std::panic::set_hook(hook);
        ready
    })
}

pub fn new_queue(seed: Option<u64>) -> Box<dyn PlayQueue> {
    match (core_queue_ready(), seed) {
        (true, Some(seed)) => Box::new(Queue::with_seed(seed)),
        (true, None) => Box::new(Queue::new()),
        (false, Some(seed)) => Box::new(LocalQueue::with_seed(seed)),
        (false, None) => Box::new(LocalQueue::new()),
    }
}

pub fn restore_queue(snapshot: QueueSnapshot) -> Box<dyn PlayQueue> {
    if core_queue_ready() {
        Box::new(Queue::from_snapshot(snapshot))
    } else {
        Box::new(LocalQueue::from_snapshot(snapshot))
    }
}

/// Minimal port of the core queue semantics, used until `feishin_core::queue::Queue` lands.
#[derive(Debug, Clone)]
pub struct LocalQueue {
    items: Vec<QueueItem>,
    shuffled: Option<Vec<usize>>,
    current: Option<Uid>,
    repeat: RepeatMode,
    next_uid: Uid,
    rng: u64,
}

impl LocalQueue {
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self::with_seed(seed)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            items: Vec::new(),
            shuffled: None,
            current: None,
            repeat: RepeatMode::None,
            next_uid: 0,
            rng: seed | 1,
        }
    }

    pub fn from_snapshot(snapshot: QueueSnapshot) -> Self {
        let mut queue = Self::new();
        queue.items = snapshot.items;
        queue.shuffled = snapshot
            .shuffled
            .filter(|order| order.len() == queue.items.len());
        queue.current = snapshot
            .current_uid
            .filter(|uid| queue.items.iter().any(|item| item.uid == *uid));
        queue.repeat = snapshot.repeat;
        queue.next_uid = snapshot.next_uid;
        queue
    }

    fn order(&self) -> Vec<usize> {
        self.shuffled
            .clone()
            .unwrap_or_else(|| (0..self.items.len()).collect())
    }

    fn index_of(&self, uid: Uid) -> Option<usize> {
        self.items.iter().position(|item| item.uid == uid)
    }

    fn current_order_pos(&self, order: &[usize]) -> Option<usize> {
        let index = self.index_of(self.current?)?;
        order.iter().position(|&i| i == index)
    }

    fn random(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    fn shuffled_order(&mut self, first: Option<usize>) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.items.len())
            .filter(|&i| Some(i) != first)
            .collect();
        for i in (1..order.len()).rev() {
            let j = (self.random() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        if let Some(first) = first {
            order.insert(0, first);
        }
        order
    }

    fn make_items(&mut self, songs: Vec<Song>) -> Vec<QueueItem> {
        songs
            .into_iter()
            .map(|song| {
                let uid = self.next_uid;
                self.next_uid += 1;
                QueueItem { uid, song }
            })
            .collect()
    }

    fn item(&self, uid: Uid) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.uid == uid)
    }

    fn peek_after_current(&self) -> Option<Uid> {
        let order = self.order();
        let next = match self.current_order_pos(&order) {
            Some(pos) if pos + 1 < order.len() => order[pos + 1],
            Some(_) if self.repeat == RepeatMode::All => *order.first()?,
            Some(_) => return None,
            None => *order.first()?,
        };
        Some(self.items[next].uid)
    }
}

impl PlayQueue for LocalQueue {
    fn display_order(&self) -> Vec<&QueueItem> {
        self.order().into_iter().map(|i| &self.items[i]).collect()
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn current(&self) -> Option<&QueueItem> {
        self.item(self.current?)
    }

    fn shuffle(&self) -> bool {
        self.shuffled.is_some()
    }

    fn set_shuffle(&mut self, on: bool) {
        if on {
            let first = self.current.and_then(|uid| self.index_of(uid));
            self.shuffled = Some(self.shuffled_order(first));
        } else {
            self.shuffled = None;
        }
    }

    fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    fn play_now(&mut self, songs: Vec<Song>, start_song_id: Option<&str>) -> Option<&QueueItem> {
        self.items = self.make_items(songs);
        let start = start_song_id
            .and_then(|id| self.items.iter().position(|item| item.song.id == id))
            .or(if self.items.is_empty() { None } else { Some(0) });
        if self.shuffled.is_some() {
            self.shuffled = Some(self.shuffled_order(start));
        }
        self.current = start.map(|i| self.items[i].uid);
        self.current()
    }

    fn next(&mut self, advance: Advance) -> Option<&QueueItem> {
        if advance == Advance::Auto && self.repeat == RepeatMode::One {
            return self.current();
        }
        let next = self.peek_after_current()?;
        self.current = Some(next);
        self.current()
    }

    fn previous(&mut self) -> Option<&QueueItem> {
        let order = self.order();
        let pos = self.current_order_pos(&order)?;
        let previous = if pos > 0 {
            order[pos - 1]
        } else if self.repeat == RepeatMode::All {
            *order.last()?
        } else {
            order[pos]
        };
        self.current = Some(self.items[previous].uid);
        self.current()
    }

    fn peek_next(&self) -> Option<&QueueItem> {
        if self.repeat == RepeatMode::One {
            return self.current();
        }
        self.item(self.peek_after_current()?)
    }

    fn play_uid(&mut self, uid: Uid) -> Option<&QueueItem> {
        self.index_of(uid)?;
        self.current = Some(uid);
        self.current()
    }

    fn remove(&mut self, uids: &[Uid]) {
        let order = self.order();
        if let Some(current) = self.current
            && uids.contains(&current)
        {
            let pos = self.current_order_pos(&order).unwrap_or(0);
            self.current = order[pos..]
                .iter()
                .chain(order[..pos].iter().rev())
                .map(|&i| self.items[i].uid)
                .find(|uid| !uids.contains(uid));
        }
        let kept_uids: Vec<Uid> = order
            .iter()
            .map(|&i| self.items[i].uid)
            .filter(|uid| !uids.contains(uid))
            .collect();
        self.items.retain(|item| !uids.contains(&item.uid));
        if self.shuffled.is_some() {
            self.shuffled = Some(
                kept_uids
                    .iter()
                    .filter_map(|&uid| self.index_of(uid))
                    .collect(),
            );
        }
    }

    fn move_items(&mut self, uids: &[Uid], before_uid: Option<Uid>) {
        if before_uid.is_some_and(|before| uids.contains(&before)) {
            return;
        }
        let order = self.order();
        let moving: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| uids.contains(&self.items[i].uid))
            .collect();
        let mut rest: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| !uids.contains(&self.items[i].uid))
            .collect();
        let at = before_uid
            .and_then(|before| rest.iter().position(|&i| self.items[i].uid == before))
            .unwrap_or(rest.len());
        rest.splice(at..at, moving);
        if self.shuffled.is_some() {
            self.shuffled = Some(rest);
        } else {
            let mut slots: Vec<Option<QueueItem>> = std::mem::take(&mut self.items)
                .into_iter()
                .map(Some)
                .collect();
            self.items = rest.iter().filter_map(|&i| slots[i].take()).collect();
        }
    }

    fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            shuffled: self.shuffled.clone(),
            current_uid: self.current,
            repeat: self.repeat,
            next_uid: self.next_uid,
        }
    }
}

impl Default for LocalQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    fn ids(queue: &dyn PlayQueue) -> Vec<String> {
        queue
            .display_order()
            .iter()
            .map(|item| item.song.id.clone())
            .collect()
    }

    fn songs(n: usize) -> Vec<Song> {
        (0..n).map(|i| test_song(&format!("s{i}"), 1000)).collect()
    }

    #[test]
    fn play_now_starts_at_song_and_shuffle_puts_it_first() {
        let mut queue = LocalQueue::with_seed(7);
        let current =
            PlayQueue::play_now(&mut queue, songs(5), Some("s2")).map(|i| i.song.id.clone());
        assert_eq!(current.as_deref(), Some("s2"));
        PlayQueue::set_shuffle(&mut queue, true);
        assert_eq!(ids(&queue)[0], "s2");
        PlayQueue::set_shuffle(&mut queue, false);
        assert_eq!(ids(&queue), ["s0", "s1", "s2", "s3", "s4"]);
    }

    #[test]
    fn move_and_remove_keep_current() {
        let mut queue = LocalQueue::with_seed(3);
        PlayQueue::play_now(&mut queue, songs(4), Some("s1"));
        let uids: Vec<Uid> = PlayQueue::display_order(&queue)
            .iter()
            .map(|i| i.uid)
            .collect();
        PlayQueue::move_items(&mut queue, &[uids[3]], Some(uids[0]));
        assert_eq!(ids(&queue), ["s3", "s0", "s1", "s2"]);
        PlayQueue::remove(&mut queue, &[uids[1]]);
        assert_eq!(ids(&queue), ["s3", "s0", "s2"]);
        assert_eq!(
            PlayQueue::current(&queue).map(|i| i.song.id.as_str()),
            Some("s2")
        );
    }

    #[test]
    fn repeat_modes() {
        let mut queue = LocalQueue::with_seed(3);
        PlayQueue::play_now(&mut queue, songs(2), Some("s1"));
        assert!(PlayQueue::peek_next(&queue).is_none());
        PlayQueue::set_repeat(&mut queue, RepeatMode::All);
        let next = PlayQueue::peek_next(&queue).map(|i| i.song.id.clone());
        assert_eq!(next.as_deref(), Some("s0"));
        PlayQueue::next(&mut queue, Advance::Manual);
        PlayQueue::set_repeat(&mut queue, RepeatMode::One);
        let auto = PlayQueue::next(&mut queue, Advance::Auto).map(|i| i.song.id.clone());
        assert_eq!(auto.as_deref(), Some("s0"));
        let manual = PlayQueue::next(&mut queue, Advance::Manual).map(|i| i.song.id.clone());
        assert_eq!(manual.as_deref(), Some("s1"));
    }
}
