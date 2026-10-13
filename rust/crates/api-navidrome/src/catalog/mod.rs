mod album_years;
mod albums;
mod annotations;
mod artists;
mod fallback_lists;
mod folders;
mod library;
mod paging;
mod play_queue;
mod playlists;
mod scanning;
mod sharing;
mod song_years;
mod songs;
mod urls;
mod year_counts;

pub(crate) use fallback_lists::FallbackLists;

const SUBSONIC_MAX_ITEMS: usize = 500;
