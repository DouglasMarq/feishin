mod account;
mod album_order;
mod albums;
mod artists;
mod catalog;
mod cover;
mod genres;
mod hashing;
mod ids;
mod lyrics;
mod music_server;
mod paging;
mod play_queue;
mod playlists;
mod records;
mod scrobbles;
mod selections;
mod server;
mod song_order;
mod songs;
mod vocabulary;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use account::demo_credential;
pub use catalog::DEMO_SONG_COUNT;
pub use cover::render_cover;
#[cfg(test)]
pub use scrobbles::ScrobbleCall;
pub use server::FakeServer;
#[cfg(test)]
pub use test_support::test_song;
