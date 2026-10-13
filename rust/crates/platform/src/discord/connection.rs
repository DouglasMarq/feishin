use std::sync::mpsc;
use std::thread;

use discord_rich_presence::activity::Activity;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::{PlatformError, Result};

const CONNECTION_THREAD_NAME: &str = "discord-rpc";
const CLOSE_OPCODE: u32 = 2;
const DELIVERY_ATTEMPTS: usize = 2;
const ERROR_EVENT_NAME: &str = "ERROR";

pub(super) enum Command {
    Set(Box<Activity<'static>>),
    Clear,
    Close,
}

#[derive(Debug, thiserror::Error)]
enum DeliveryFailure {
    #[error("{0}")]
    Ipc(#[from] discord_rich_presence::error::Error),
    #[error("connection closed by Discord: {0}")]
    ClosedByDiscord(String),
}

pub(super) fn spawn_connection_thread(
    mut client: DiscordIpcClient,
    command_receiver: mpsc::Receiver<Command>,
    ready_sender: mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    thread::Builder::new()
        .name(CONNECTION_THREAD_NAME.to_owned())
        .spawn(move || {
            let connected = client
                .connect()
                .map_err(|error| PlatformError::Discord(error.to_string()));
            let is_connected = connected.is_ok();
            report_connection_result(&ready_sender, connected);
            if is_connected {
                run(client, command_receiver);
            }
        })
        .map(drop)
        .map_err(|error| PlatformError::Discord(error.to_string()))
}

fn report_connection_result(ready_sender: &mpsc::SyncSender<Result<()>>, connected: Result<()>) {
    if ready_sender.send(connected).is_err() {
        tracing::debug!(
            "Discord RPC connection result arrived after DiscordPresence::connect stopped waiting"
        );
    }
}

fn run(mut client: DiscordIpcClient, command_receiver: mpsc::Receiver<Command>) {
    let mut is_connected = true;
    while let Ok(first_command) = command_receiver.recv() {
        let command = latest_pending_command(first_command, &command_receiver);
        if let Command::Close = command {
            close(&mut client, is_connected);
            return;
        }
        deliver(&mut client, &mut is_connected, &command);
    }
}

fn latest_pending_command(
    first_command: Command,
    command_receiver: &mpsc::Receiver<Command>,
) -> Command {
    command_receiver
        .try_iter()
        .fold(first_command, |current, next| match current {
            Command::Close => current,
            Command::Set(_) | Command::Clear => next,
        })
}

fn close(client: &mut DiscordIpcClient, is_connected: bool) {
    if is_connected && let Err(error) = client.close() {
        tracing::debug!(%error, "Discord RPC close failed");
    }
}

fn deliver(client: &mut DiscordIpcClient, is_connected: &mut bool, command: &Command) {
    for _attempt in 0..DELIVERY_ATTEMPTS {
        if !ensure_connected(client, is_connected) {
            return;
        }
        match send_command(client, command) {
            Ok(()) => return,
            Err(failure) => {
                tracing::warn!(error = %failure, "Discord RPC update failed");
                *is_connected = false;
            }
        }
    }
}

fn ensure_connected(client: &mut DiscordIpcClient, is_connected: &mut bool) -> bool {
    if *is_connected {
        return true;
    }
    match client.connect() {
        Ok(()) => {
            *is_connected = true;
            true
        }
        Err(error) => {
            tracing::warn!(%error, "Discord RPC connect failed");
            false
        }
    }
}

fn send_command(
    client: &mut DiscordIpcClient,
    command: &Command,
) -> std::result::Result<(), DeliveryFailure> {
    match command {
        Command::Set(activity) => client.set_activity((**activity).clone()),
        Command::Clear => client.clear_activity(),
        Command::Close => return Ok(()),
    }?;
    let (opcode, response) = client.recv()?;
    if opcode == CLOSE_OPCODE {
        return Err(DeliveryFailure::ClosedByDiscord(response.to_string()));
    }
    let event_name = response
        .get("evt")
        .and_then(|event_name| event_name.as_str());
    if event_name == Some(ERROR_EVENT_NAME) {
        tracing::warn!(%response, "Discord RPC rejected the activity");
    }
    Ok(())
}
