use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::controller::Command;

const TICK: Duration = Duration::from_secs(1);

pub(super) fn spawn_ticker(commands: UnboundedSender<Command>) -> AbortHandle {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + TICK, TICK);
        let mut previous_tick = Instant::now();
        loop {
            interval.tick().await;
            let now = Instant::now();
            let elapsed_ms = now.duration_since(previous_tick).as_millis() as u64;
            previous_tick = now;
            if commands.send(Command::SleepTick { elapsed_ms }).is_err() {
                return;
            }
        }
    })
    .abort_handle()
}
