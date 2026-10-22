use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};

use tokio::sync::oneshot;

use crate::protocol::{DeviceId, DeviceInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    pub device: DeviceInfo,
    pub address: IpAddr,
}

struct OpenSession {
    id: u64,
    controller: Controller,
    stop: Option<oneshot::Sender<()>>,
}

#[derive(Default)]
pub(crate) struct Controllers {
    sessions: Mutex<Vec<OpenSession>>,
    next_id: Mutex<u64>,
}

impl Controllers {
    pub(crate) fn open(&self, controller: Controller) -> (u64, oneshot::Receiver<()>) {
        let id = {
            let mut next_id = self.next_id.lock().unwrap_or_else(PoisonError::into_inner);
            *next_id += 1;
            *next_id
        };
        let (stop, stopped) = oneshot::channel();
        self.sessions().push(OpenSession {
            id,
            controller,
            stop: Some(stop),
        });
        (id, stopped)
    }

    pub(crate) fn close(&self, id: u64) {
        self.sessions().retain(|session| session.id != id);
    }

    pub(crate) fn stop(&self, device: &DeviceId) -> Option<Controller> {
        let mut stopped = None;
        for session in self
            .sessions()
            .iter_mut()
            .filter(|session| session.controller.device.id == *device)
        {
            stop_session(session);
            stopped = Some(session.controller.clone());
        }
        stopped
    }

    pub(crate) fn stop_all(&self) {
        self.sessions().iter_mut().for_each(stop_session);
    }

    pub(crate) fn list(&self) -> Vec<Controller> {
        self.sessions()
            .iter()
            .map(|session| session.controller.clone())
            .collect()
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, Vec<OpenSession>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn stop_session(session: &mut OpenSession) {
    if let Some(stop) = session.stop.take()
        && stop.send(()).is_err()
    {
        tracing::debug!("the remote control session had already ended");
    }
}
