use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

#[derive(Clone, Default)]
pub struct SendOrder {
    last_turn: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

pub struct SendTurn {
    previous: Option<oneshot::Receiver<()>>,
    _finished: oneshot::Sender<()>,
}

impl SendOrder {
    pub fn take_turn(&self) -> SendTurn {
        let (finished, finished_signal) = oneshot::channel();
        let previous = match self.last_turn.lock() {
            Ok(mut last_turn) => last_turn.replace(finished_signal),
            Err(error) => {
                tracing::warn!("the Discover send order is broken: {error}");
                None
            }
        };
        SendTurn {
            previous,
            _finished: finished,
        }
    }
}

impl SendTurn {
    pub async fn wait_for_earlier_turns(&mut self) {
        if let Some(previous) = self.previous.take() {
            previous.await.ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn later_turns_wait_for_earlier_ones_to_finish() {
        let order = SendOrder::default();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut first = order.take_turn();
        let mut second = order.take_turn();
        let second_sender = sender.clone();
        let second_task = tokio::spawn(async move {
            second.wait_for_earlier_turns().await;
            second_sender.send("second").unwrap();
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        first.wait_for_earlier_turns().await;
        sender.send("first").unwrap();
        drop(first);
        second_task.await.unwrap();
        assert_eq!(receiver.recv().await, Some("first"));
        assert_eq!(receiver.recv().await, Some("second"));
    }
}
