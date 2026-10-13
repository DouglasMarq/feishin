mod auth;
mod client;
mod cookie_export;
mod countries;
mod error;
mod locale;
mod model;
mod raw;
mod request;

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod cookie_export_tests;

pub use auth::MusicAuth;
pub use client::{MUSIC_API_BASE, MusicClient};
pub use cookie_export::{private_runtime_dir, sign_in_with_browser};
pub use countries::{
    CHART_COUNTRIES, ChartCountry, FALLBACK_COUNTRY, GLOBAL_CHARTS, chart_country,
    chart_country_or_fallback,
};
pub use error::MusicError;
pub use locale::{MusicLocale, SystemLocale};
pub use model::{
    CollectionKind, CollectionPage, MoodCategory, MusicCollection, MusicShelf, MusicSong,
    ShelfItem, collection_kind, collection_url, is_browse_id,
};
