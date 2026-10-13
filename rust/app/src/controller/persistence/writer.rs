use std::sync::Arc;

use feishin_storage::queue_store::{EncodedQueue, PlaybackState};
use tokio::sync::{mpsc, oneshot};

use super::{Command, PersistenceEvent, forward_command};
use crate::store::Store;

pub(super) enum QueueFile {
    Queue {
        encoded: EncodedQueue,
        revision: u64,
    },
    Playback(PlaybackState),
}

struct WriteJob {
    file: QueueFile,
    written: oneshot::Sender<()>,
}

#[derive(Clone)]
struct WriterTarget {
    store: Arc<dyn Store>,
    commands: mpsc::UnboundedSender<Command>,
}

pub(super) struct QueueFileWriter {
    target: WriterTarget,
    jobs: Option<mpsc::UnboundedSender<WriteJob>>,
}

impl QueueFileWriter {
    pub(super) fn new(store: Arc<dyn Store>, commands: mpsc::UnboundedSender<Command>) -> Self {
        Self {
            target: WriterTarget { store, commands },
            jobs: None,
        }
    }

    pub(super) fn write(&mut self, file: QueueFile) -> oneshot::Receiver<()> {
        let (written, finished) = oneshot::channel();
        let target = &self.target;
        let jobs = self
            .jobs
            .get_or_insert_with(|| spawn_writer(target.clone()));
        if jobs.send(WriteJob { file, written }).is_err() {
            tracing::warn!("the queue writer has stopped, the queue was not saved");
        }
        finished
    }
}

fn spawn_writer(target: WriterTarget) -> mpsc::UnboundedSender<WriteJob> {
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(write_in_order(target, receiver));
    sender
}

async fn write_in_order(target: WriterTarget, mut jobs: mpsc::UnboundedReceiver<WriteJob>) {
    while let Some(WriteJob { file, written }) = jobs.recv().await {
        let failed_revision = write_off_the_runtime(&target, file).await;
        if let Some(revision) = failed_revision {
            forward_command(
                &target.commands,
                Command::Persistence(PersistenceEvent::QueueSaveFailed(revision)),
            );
        }
        if written.send(()).is_err() {
            tracing::trace!("nobody waits for this queue save");
        }
    }
}

async fn write_off_the_runtime(target: &WriterTarget, file: QueueFile) -> Option<u64> {
    let store = Arc::clone(&target.store);
    let revision = match &file {
        QueueFile::Queue { revision, .. } => Some(*revision),
        QueueFile::Playback(_) => None,
    };
    let blocking_write = tokio::task::spawn_blocking(move || write_file(store.as_ref(), &file));
    match blocking_write.await {
        Ok(true) => None,
        Ok(false) => revision,
        Err(error) => {
            tracing::warn!("the queue save task failed: {error}");
            revision
        }
    }
}

fn write_file(store: &dyn Store, file: &QueueFile) -> bool {
    let saved = match file {
        QueueFile::Queue { encoded, .. } => store.save_queue(encoded),
        QueueFile::Playback(playback) => store.save_playback(playback),
    };
    saved
        .inspect_err(|error| tracing::warn!("saving the queue failed: {error:#}"))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::Queue;

    use super::*;
    use crate::store::JsonStore;

    #[tokio::test]
    async fn a_failed_queue_write_reports_its_revision() {
        let directory = tempfile::tempdir().unwrap();
        let not_a_directory = directory.path().join("file");
        std::fs::write(&not_a_directory, b"").unwrap();
        let (commands, mut received) = mpsc::unbounded_channel();
        let store = Arc::new(JsonStore::new(not_a_directory));
        let mut writer = QueueFileWriter::new(store, commands);
        let encoded =
            feishin_storage::queue_store::encode(&Queue::with_seed(1).snapshot_ref(), 0).unwrap();
        let playback = encoded.playback();

        writer
            .write(QueueFile::Queue {
                encoded,
                revision: 7,
            })
            .await
            .unwrap();
        assert!(matches!(
            received.try_recv(),
            Ok(Command::Persistence(PersistenceEvent::QueueSaveFailed(7)))
        ));
        writer.write(QueueFile::Playback(playback)).await.unwrap();
        assert!(received.try_recv().is_err());
    }
}
