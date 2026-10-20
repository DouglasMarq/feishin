mod demo_source;
mod feed;
mod library_match;
pub mod mixes;
mod names;
mod sign_in;
mod source;
mod tracks;
mod youtube_music;

#[cfg(test)]
mod tests;

pub use demo_source::DemoDiscover;
pub use feed::{DiscoverFeed, FeedNotice, FeedRequest, default_chart_country};
pub use library_match::{LibraryMatcher, LibraryPresence};
#[cfg(test)]
pub use sign_in::SignIn;
pub use source::DiscoverSource;
pub use tracks::resolve_tracks;
pub use youtube_music::YouTubeMusicSource;
