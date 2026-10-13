mod demo_source;
mod feed;
mod library_match;
mod names;
mod sign_in;
mod source;
mod tracks;
mod youtube_music;

#[cfg(test)]
mod tests;

pub use demo_source::DemoDiscover;
#[cfg(test)]
pub use feed::FeedNotice;
pub use feed::{DiscoverFeed, FeedRequest, default_chart_country};
pub use library_match::LibraryMatcher;
#[cfg(test)]
pub use sign_in::SignIn;
pub use source::DiscoverSource;
pub use tracks::resolve_tracks;
pub use youtube_music::YouTubeMusicSource;
