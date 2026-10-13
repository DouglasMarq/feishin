use feishin_core::model::Song;
use tokio::sync::oneshot;

use super::message::{StreamEvent, StreamId, StreamMessage, StreamPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

pub type Deliver = Box<dyn Fn(StreamMessage) + Send + Sync>;

pub struct Delivery {
    stream: StreamId,
    deliver: Deliver,
    pending: Option<oneshot::Receiver<()>>,
}

impl Delivery {
    pub fn new(stream: StreamId, deliver: Deliver) -> Self {
        Self {
            stream,
            deliver,
            pending: None,
        }
    }

    pub async fn page(
        &mut self,
        songs: Vec<Song>,
        event: impl FnOnce(StreamPage) -> StreamEvent,
    ) -> Result<(), Cancelled> {
        self.wait_until_applied().await?;
        let (page, applied) = StreamPage::new(songs);
        self.send(event(page));
        self.pending = Some(applied);
        Ok(())
    }

    pub async fn wait_until_applied(&mut self) -> Result<(), Cancelled> {
        match self.pending.take() {
            Some(applied) => applied.await.map_err(|_| Cancelled),
            None => Ok(()),
        }
    }

    pub fn send(&self, event: StreamEvent) {
        (self.deliver)(StreamMessage {
            stream: self.stream,
            event,
        });
    }
}
