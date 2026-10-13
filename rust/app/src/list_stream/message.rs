use feishin_core::model::Song;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Now,
    Next,
    Last,
}

#[derive(Debug)]
pub struct Confirmation(oneshot::Sender<()>);

impl Confirmation {
    pub fn confirm(self) {
        if self.0.send(()).is_err() {
            tracing::debug!("the list stream stopped before its page was applied");
        }
    }
}

#[derive(Debug)]
pub struct StreamPage {
    pub songs: Vec<Song>,
    pub applied: Confirmation,
}

impl StreamPage {
    pub fn new(songs: Vec<Song>) -> (Self, oneshot::Receiver<()>) {
        let (sender, receiver) = oneshot::channel();
        let page = Self {
            songs,
            applied: Confirmation(sender),
        };
        (page, receiver)
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    Start {
        page: StreamPage,
        start_index: usize,
    },
    After(StreamPage),
    Before(StreamPage),
    Failed(String),
    Finished,
}

#[derive(Debug)]
pub struct StreamMessage {
    pub stream: StreamId,
    pub event: StreamEvent,
}
