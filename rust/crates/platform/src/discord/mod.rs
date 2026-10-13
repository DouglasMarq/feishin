mod activity;
mod connection;
mod image_url;
mod options;
mod text_field;

use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::DiscordIpcClient;

use crate::{NowPlaying, PlatformError, Result};
use activity::activity_for;
use connection::{Command, spawn_connection_thread};
pub use options::{DEFAULT_DISCORD_CLIENT_ID, DiscordDisplayType, DiscordOptions};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct DiscordPresence {
    commands: mpsc::Sender<Command>,
    options: DiscordOptions,
}

impl DiscordPresence {
    pub fn connect(client_id: &str) -> Result<Self> {
        let client = DiscordIpcClient::new(client_id_or_default(client_id));
        let (commands, command_receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        spawn_connection_thread(client, command_receiver, ready_sender)?;
        match ready_receiver.recv_timeout(CONNECT_TIMEOUT) {
            Ok(connected) => connected.map(|()| Self {
                commands,
                options: DiscordOptions::default(),
            }),
            Err(_) => Err(PlatformError::Discord(
                "timed out connecting to Discord".to_owned(),
            )),
        }
    }

    pub fn options(&self) -> &DiscordOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: DiscordOptions) {
        self.options = options;
    }

    pub fn set_activity(
        &mut self,
        now_playing: &NowPlaying,
        position_ms: Option<u64>,
        paused: bool,
    ) -> Result<()> {
        let activity = activity_for(
            now_playing,
            position_ms,
            paused,
            &self.options,
            unix_now_ms(),
        );
        let command = match activity {
            Some(activity) => Command::Set(Box::new(activity)),
            None => Command::Clear,
        };
        self.send(command)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.send(Command::Clear)
    }

    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| PlatformError::Discord("Discord thread is not running".to_owned()))
    }
}

impl Drop for DiscordPresence {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Close);
    }
}

fn client_id_or_default(client_id: &str) -> &str {
    match client_id.trim() {
        "" => DEFAULT_DISCORD_CLIENT_ID,
        trimmed_client_id => trimmed_client_id,
    }
}

fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
