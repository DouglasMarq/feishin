use super::browser::open_in_browser;
use super::deps::DiscoverDeps;
use super::model::with_discover;
use super::playing::{Placement, play_songs, queued_message, send_resolved};
use crate::ui::AppWindow;

fn page_songs(window: &AppWindow) -> Vec<feishin_youtube::music::MusicSong> {
    with_discover(window, |model| model.page.borrow().songs.clone()).unwrap_or_default()
}

pub fn play_page_from(window: &AppWindow, deps: &DiscoverDeps, index: usize) {
    let songs = page_songs(window);
    if index < songs.len() {
        play_songs(window, deps, songs, Placement::PlayFrom(index));
    }
}

pub fn place_page_song(
    window: &AppWindow,
    deps: &DiscoverDeps,
    index: usize,
    placement: Placement,
) {
    if let Some(song) = with_discover(window, |model| model.page_song(index)).flatten() {
        play_songs(window, deps, vec![song], placement);
    }
}

pub fn queue_page(window: &AppWindow, deps: &DiscoverDeps) {
    let songs = page_songs(window);
    let title =
        with_discover(window, |model| model.page.borrow().title.clone()).unwrap_or_default();
    let message = queued_message(Placement::Last, &songs, Some(&title));
    send_resolved(window, deps, songs, Placement::Last, message);
}

pub fn open_song_in_browser(window: &AppWindow, deps: &DiscoverDeps, index: usize) {
    if let Some(song) = with_discover(window, |model| model.page_song(index)).flatten() {
        open_in_browser(window, deps, song.web_url());
    }
}

pub fn open_page_in_browser(window: &AppWindow, deps: &DiscoverDeps) {
    let url = with_discover(window, |model| {
        model
            .page
            .borrow()
            .target
            .as_ref()
            .and_then(|target| target.web_url())
    })
    .flatten();
    if let Some(url) = url {
        open_in_browser(window, deps, url);
    }
}
