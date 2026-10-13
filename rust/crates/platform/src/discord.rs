use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::{NowPlaying, PlatformError, Result};

/// Feishin's Discord application.
pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1165957668758900787";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FIELD_LENGTH: usize = 127;
const MAX_IMAGE_URL_LENGTH: usize = 256;
const DEFAULT_LARGE_IMAGE: &str = "icon";
const CLOSE_OPCODE: u32 = 2;
const CREDENTIAL_QUERY_KEYS: [&str; 7] = ["u", "p", "t", "s", "apikey", "api_key", "token"];

/// Which field Discord shows next to the user name in the member list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscordDisplayType {
    #[default]
    Feishin,
    ArtistName,
    SongName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordOptions {
    pub display_type: DiscordDisplayType,
    /// Listening when true, Playing otherwise.
    pub show_as_listening: bool,
    /// When false, a paused track clears the activity.
    pub show_paused: bool,
    pub show_state_icon: bool,
}

impl Default for DiscordOptions {
    fn default() -> Self {
        Self {
            display_type: DiscordDisplayType::Feishin,
            show_as_listening: true,
            show_paused: true,
            show_state_icon: true,
        }
    }
}

enum Command {
    Set(Box<Activity<'static>>),
    Clear,
    Close,
}

/// Discord Rich Presence over the local IPC socket.
///
/// Updates are delivered by a background thread that reconnects on demand, so
/// a Discord restart or a failed update never surfaces as an error here.
pub struct DiscordPresence {
    commands: mpsc::Sender<Command>,
    options: DiscordOptions,
}

impl DiscordPresence {
    /// An empty `client_id` uses [`DEFAULT_DISCORD_CLIENT_ID`]. Fails when Discord is not running.
    pub fn connect(client_id: &str) -> Result<Self> {
        let client_id = match client_id.trim() {
            "" => DEFAULT_DISCORD_CLIENT_ID,
            id => id,
        };
        let mut client = DiscordIpcClient::new(client_id);
        let (commands, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        thread::Builder::new()
            .name("discord-rpc".to_owned())
            .spawn(move || {
                let connected = client.connect().map_err(|err| err.to_string());
                let ok = connected.is_ok();
                let _ = ready_tx.send(connected);
                if ok {
                    run(client, rx);
                }
            })
            .map_err(|err| PlatformError::Discord(err.to_string()))?;

        match ready_rx.recv_timeout(CONNECT_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                commands,
                options: DiscordOptions::default(),
            }),
            Ok(Err(err)) => Err(PlatformError::Discord(err)),
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
        let command = match activity_for(
            now_playing,
            position_ms,
            paused,
            &self.options,
            unix_now_ms(),
        ) {
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

fn run(mut client: DiscordIpcClient, rx: mpsc::Receiver<Command>) {
    let mut connected = true;
    while let Ok(first) = rx.recv() {
        let command = rx.try_iter().fold(first, |current, next| match current {
            Command::Close => current,
            _ => next,
        });
        if let Command::Close = command {
            if connected {
                let _ = client.close();
            }
            return;
        }
        deliver(&mut client, &mut connected, &command);
    }
}

fn deliver(client: &mut DiscordIpcClient, connected: &mut bool, command: &Command) {
    for _ in 0..2 {
        if !*connected {
            if let Err(err) = client.connect() {
                tracing::warn!(%err, "Discord RPC connect failed");
                return;
            }
            *connected = true;
        }
        match send_command(client, command) {
            Ok(()) => return,
            Err(err) => {
                tracing::warn!(%err, "Discord RPC update failed");
                *connected = false;
            }
        }
    }
}

fn send_command(
    client: &mut DiscordIpcClient,
    command: &Command,
) -> std::result::Result<(), String> {
    match command {
        Command::Set(activity) => client.set_activity((**activity).clone()),
        Command::Clear => client.clear_activity(),
        Command::Close => return Ok(()),
    }
    .map_err(|err| err.to_string())?;

    let (opcode, response) = client.recv().map_err(|err| err.to_string())?;
    if opcode == CLOSE_OPCODE {
        return Err(format!("connection closed by Discord: {response}"));
    }
    if response.get("evt").and_then(|evt| evt.as_str()) == Some("ERROR") {
        tracing::warn!(%response, "Discord RPC rejected the activity");
    }
    Ok(())
}

fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn truncate(field: &str) -> String {
    if field.chars().count() <= MAX_FIELD_LENGTH {
        return field.to_owned();
    }
    let mut truncated: String = field.chars().take(MAX_FIELD_LENGTH - 1).collect();
    truncated.push('\u{2026}');
    truncated
}

fn text_field(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        return fallback.to_owned();
    }
    let mut padded = value.to_owned();
    for _ in value.chars().count()..2 {
        padded.push(' ');
    }
    truncate(&padded)
}

/// Discord shows the image publicly, so URLs carrying credentials are refused.
fn is_shareable_image_url(url: &str) -> bool {
    let is_https = url
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"));
    if !is_https || url.len() > MAX_IMAGE_URL_LENGTH {
        return false;
    }
    let query = url.split_once('?').map_or("", |(_, query)| query);
    let query = query.split_once('#').map_or(query, |(query, _)| query);
    !query
        .split('&')
        .filter_map(|pair| pair.split('=').next())
        .any(|key| {
            CREDENTIAL_QUERY_KEYS
                .iter()
                .any(|credential| key.eq_ignore_ascii_case(credential))
        })
}

fn activity_for(
    now_playing: &NowPlaying,
    position_ms: Option<u64>,
    paused: bool,
    options: &DiscordOptions,
    now_ms: i64,
) -> Option<Activity<'static>> {
    if now_playing.track_id.is_empty() || (paused && !options.show_paused) {
        return None;
    }

    let large_image = now_playing
        .cover_url
        .as_deref()
        .filter(|url| is_shareable_image_url(url))
        .unwrap_or(DEFAULT_LARGE_IMAGE)
        .to_owned();
    let mut assets = Assets::new()
        .large_image(large_image)
        .large_text(text_field(
            now_playing.album.as_deref().unwrap_or_default(),
            "Unknown album",
        ));
    let state_icon = if paused {
        Some(("paused", "Paused"))
    } else if options.show_state_icon {
        Some(("playing", "Playing"))
    } else {
        None
    };
    if let Some((key, text)) = state_icon {
        assets = assets.small_image(key).small_text(text);
    }

    let mut activity = Activity::new()
        .details(text_field(&now_playing.title, "Idle"))
        .state(text_field(
            &now_playing.artists.join(", "),
            "Unknown artist",
        ))
        .assets(assets)
        .activity_type(if options.show_as_listening {
            ActivityType::Listening
        } else {
            ActivityType::Playing
        })
        .status_display_type(match options.display_type {
            DiscordDisplayType::Feishin => StatusDisplayType::Name,
            DiscordDisplayType::ArtistName => StatusDisplayType::State,
            DiscordDisplayType::SongName => StatusDisplayType::Details,
        });

    if !paused && let Some(position_ms) = position_ms {
        let start = now_ms.saturating_sub(i64::try_from(position_ms).unwrap_or(i64::MAX));
        let mut timestamps = Timestamps::new().start(start);
        if let Some(duration_ms) = now_playing
            .duration_ms
            .filter(|ms| *ms > 0)
            .and_then(|ms| i64::try_from(ms).ok())
        {
            timestamps = timestamps.end(start.saturating_add(duration_ms));
        }
        activity = activity.timestamps(timestamps);
    }

    Some(activity)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn song() -> NowPlaying {
        NowPlaying {
            track_id: "t1".to_owned(),
            title: "Song".to_owned(),
            artists: vec!["Artist A".to_owned(), "Artist B".to_owned()],
            album: Some("Album".to_owned()),
            duration_ms: Some(200_000),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
        }
    }

    fn render(
        now_playing: &NowPlaying,
        position_ms: Option<u64>,
        paused: bool,
        options: &DiscordOptions,
    ) -> Option<Value> {
        activity_for(now_playing, position_ms, paused, options, NOW)
            .map(|activity| serde_json::to_value(activity).unwrap())
    }

    #[test]
    fn playing_activity() {
        let value = render(&song(), Some(30_000), false, &DiscordOptions::default()).unwrap();
        assert_eq!(
            value,
            json!({
                "details": "Song",
                "state": "Artist A, Artist B",
                "type": 2,
                "status_display_type": 0,
                "assets": {
                    "large_image": "https://example.com/cover.jpg",
                    "large_text": "Album",
                    "small_image": "playing",
                    "small_text": "Playing",
                },
                "timestamps": {
                    "start": NOW - 30_000,
                    "end": NOW - 30_000 + 200_000,
                },
            })
        );
    }

    #[test]
    fn paused_activity_has_no_timestamps() {
        let value = render(&song(), Some(30_000), true, &DiscordOptions::default()).unwrap();
        assert!(value.get("timestamps").is_none());
        assert_eq!(value["assets"]["small_image"], "paused");
        assert_eq!(value["assets"]["small_text"], "Paused");
    }

    #[test]
    fn paused_is_cleared_when_hidden() {
        let options = DiscordOptions {
            show_paused: false,
            ..DiscordOptions::default()
        };
        assert!(render(&song(), Some(0), true, &options).is_none());
        assert!(render(&song(), Some(0), false, &options).is_some());
    }

    #[test]
    fn no_track_clears() {
        assert!(
            render(
                &NowPlaying::default(),
                None,
                false,
                &DiscordOptions::default()
            )
            .is_none()
        );
    }

    #[test]
    fn fallbacks_and_padding() {
        let now_playing = NowPlaying {
            track_id: "t1".to_owned(),
            title: "A".to_owned(),
            duration_ms: None,
            ..NowPlaying::default()
        };
        let options = DiscordOptions {
            show_state_icon: false,
            show_as_listening: false,
            display_type: DiscordDisplayType::SongName,
            ..DiscordOptions::default()
        };
        let value = render(&now_playing, Some(1_000), false, &options).unwrap();
        assert_eq!(value["details"], "A ");
        assert_eq!(value["state"], "Unknown artist");
        assert_eq!(value["type"], 0);
        assert_eq!(value["status_display_type"], 2);
        assert_eq!(
            value["assets"],
            json!({ "large_image": "icon", "large_text": "Unknown album" })
        );
        assert_eq!(value["timestamps"], json!({ "start": NOW - 1_000 }));
    }

    #[test]
    fn long_fields_are_truncated() {
        let long = "x".repeat(200);
        let truncated = text_field(&long, "Idle");
        assert_eq!(truncated.chars().count(), MAX_FIELD_LENGTH);
        assert!(truncated.ends_with('\u{2026}'));
        assert_eq!(text_field(&"y".repeat(127), "Idle").chars().count(), 127);
    }

    #[test]
    fn image_url_filter() {
        assert!(is_shareable_image_url("https://example.com/a.jpg"));
        assert!(is_shareable_image_url("HTTPS://example.com/a.jpg?size=300"));
        assert!(!is_shareable_image_url("http://example.com/a.jpg"));
        assert!(!is_shareable_image_url("icon"));
        assert!(!is_shareable_image_url(
            "https://nd.example.com/rest/getCoverArt?id=1&u=me&t=abc&s=salt"
        ));
        assert!(!is_shareable_image_url(
            "https://jf.example.com/Items/1/Images/Primary?api_key=secret"
        ));
        let long = format!("https://example.com/{}", "a".repeat(250));
        assert!(!is_shareable_image_url(&long));
    }
}
