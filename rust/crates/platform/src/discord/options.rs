pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1165957668758900787";

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
    pub show_as_listening: bool,
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
