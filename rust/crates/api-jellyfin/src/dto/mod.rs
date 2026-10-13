mod authentication;
mod library;
mod lyrics;
mod media;
mod requests;
mod scheduled_task;

use serde::{Deserialize, Deserializer};

pub use authentication::AuthResponse;
pub use library::{
    Album, AlbumArtist, CreatedPlaylist, Genre, ItemList, MusicFolder, NamedItem, Playlist,
};
pub use lyrics::{LyricLine, Lyrics};
pub use media::{ImageTags, MediaSource, Song};
pub use requests::{AuthenticateByName, EmptyBody, NewPlaylist, PlaybackEvent, PlaybackReport};
pub use scheduled_task::ScheduledTask;

fn null_as_default<'de, Source, Field>(deserializer: Source) -> Result<Field, Source::Error>
where
    Source: Deserializer<'de>,
    Field: Default + Deserialize<'de>,
{
    Ok(Option::<Field>::deserialize(deserializer)?.unwrap_or_default())
}
