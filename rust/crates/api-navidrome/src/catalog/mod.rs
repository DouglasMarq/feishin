mod albums;
mod annotations;
mod artists;
mod fallback_lists;
mod library;
mod paging;
mod play_queue;
mod playlists;
mod scanning;
mod songs;
mod urls;

pub(crate) use fallback_lists::FallbackLists;

const SUBSONIC_MAX_ITEMS: usize = 500;
