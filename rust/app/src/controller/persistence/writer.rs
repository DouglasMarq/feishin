use std::sync::Arc;

use feishin_storage::queue_store::{EncodedQueue, PlaybackState};
use tokio::sync::{mpsc, oneshot};

use crate::store::Store;

pub(super) enum QueueFile {
    Queue(EncodedQueue),
    Playback(PlaybackState),
}

struct WriteJob {
    file: QueueFile,
    written: oneshot::Sender<()>,
}

pub(super) struct QueueFileWriter {
    store: Arc<dyn Store>,
    jobs: Option<mpsc::UnboundedSender<WriteJob>>,
}

impl QueueFileWriter {
    pub(super) fn new(store: Arc<dyn Store>) -> Self {
        Self { store, jobs: None }
    }

    pub(super) fn write(&mut self, file: QueueFile) -> oneshot::Receiver<()> {
        let (written, finished) = oneshot::channel();
        let store = &self.store;
        let jobs = self
            .jobs
            .get_or_insert_with(|| spawn_writer(Arc::clone(store)));
        if jobs.send(WriteJob { file, written }).is_err() {
            tracing::warn!("the queue writer has stopped, the queue was not saved");
        }
        finished
    }
}

fn spawn_writer(store: Arc<dyn Store>) -> mpsc::UnboundedSender<WriteJob> {
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(write_in_order(store, receiver));
    sender
}

async fn write_in_order(store: Arc<dyn Store>, mut jobs: mpsc::UnboundedReceiver<WriteJob>) {
    while let Some(WriteJob { file, written }) = jobs.recv().await {
        let store = Arc::clone(&store);
        let blocking_write = tokio::task::spawn_blocking(move || write_file(store.as_ref(), &file));
        if let Err(error) = blocking_write.await {
            tracing::warn!("the queue save task failed: {error}");
        }
        if written.send(()).is_err() {
            tracing::trace!("nobody waits for this queue save");
        }
    }
}

fn write_file(store: &dyn Store, file: &QueueFile) {
    let saved = match file {
        QueueFile::Queue(encoded) => store.save_queue(encoded),
        QueueFile::Playback(playback) => store.save_playback(playback),
    };
    if let Err(error) = saved {
        tracing::warn!("saving the queue failed: {error:#}");
    }
}
