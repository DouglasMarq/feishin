use std::future::Future;
use std::sync::{Arc, RwLock};

use feishin_api::MusicServer;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Session {
    server: Arc<RwLock<Option<Arc<dyn MusicServer>>>>,
    work_epoch: Arc<watch::Sender<u64>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            server: Arc::default(),
            work_epoch: Arc::new(watch::Sender::new(0)),
        }
    }
}

impl Session {
    pub fn get(&self) -> Option<Arc<dyn MusicServer>> {
        self.server.read().ok().and_then(|server| server.clone())
    }

    pub fn set(&self, server: Option<Arc<dyn MusicServer>>) {
        if let Ok(mut current) = self.server.write() {
            *current = server;
        }
        self.end_background_work();
    }

    pub fn end_background_work(&self) {
        self.work_epoch
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }

    pub fn background_work(&self) -> BackgroundWork {
        BackgroundWork {
            epoch: self.work_epoch.subscribe(),
        }
    }
}

pub struct BackgroundWork {
    epoch: watch::Receiver<u64>,
}

impl BackgroundWork {
    pub async fn run<Output>(mut self, work: impl Future<Output = Output>) -> Option<Output> {
        tokio::select! {
            output = work => Some(output),
            () = self.ended() => {
                tracing::info!("stopped background work because the session ended");
                None
            }
        }
    }

    async fn ended(&mut self) {
        if self.epoch.changed().await.is_err() {
            tracing::debug!("the session is gone, ending its background work");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::demo::FakeServer;

    async fn count_forever(steps: &AtomicUsize) {
        loop {
            steps.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    async fn stops_when(session: &Session, end: impl FnOnce(&Session)) {
        let steps = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&steps);
        let work = session.background_work();
        let task = tokio::spawn(async move { work.run(count_forever(&counting)).await });
        tokio::time::sleep(Duration::from_millis(5)).await;
        end(session);
        let stopped = tokio::time::timeout(Duration::from_secs(5), task).await;
        assert!(matches!(stopped, Ok(Ok(None))));
        let steps_at_stop = steps.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(steps.load(Ordering::SeqCst), steps_at_stop);
    }

    #[tokio::test]
    async fn logging_out_stops_background_work() {
        let session = Session::default();
        session.set(Some(Arc::new(FakeServer::new(10))));
        stops_when(&session, |session| session.set(None)).await;
    }

    #[tokio::test]
    async fn shutting_down_stops_background_work_but_keeps_the_server() {
        let session = Session::default();
        session.set(Some(Arc::new(FakeServer::new(10))));
        stops_when(&session, Session::end_background_work).await;
        assert!(session.get().is_some());
    }

    #[tokio::test]
    async fn work_that_finishes_first_returns_its_output() {
        let session = Session::default();
        let output = session.background_work().run(async { 7 }).await;
        assert_eq!(output, Some(7));
        let later = session.background_work();
        session.end_background_work();
        assert_eq!(later.run(std::future::pending::<u8>()).await, None);
    }
}
