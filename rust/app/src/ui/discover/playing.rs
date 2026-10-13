use std::sync::Arc;
use std::sync::atomic::Ordering;

use feishin_core::queue::QueueTrack;
use feishin_youtube::music::MusicSong;
use slint::ComponentHandle;

use super::deps::DiscoverDeps;
use crate::controller::{Command, TrackCommand};
use crate::discover::resolve_tracks;
use crate::ui::{AppWindow, on_ui, show_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    PlayFrom(usize),
    Next,
    Last,
}

pub fn command_for(placement: Placement, tracks: Vec<QueueTrack>) -> Command {
    Command::Tracks(match placement {
        Placement::PlayFrom(start) => TrackCommand::PlayNow {
            tracks,
            start_index: Some(start),
        },
        Placement::Next => TrackCommand::AddNext(tracks),
        Placement::Last => TrackCommand::AddLast(tracks),
    })
}

pub fn queued_message(
    placement: Placement,
    songs: &[MusicSong],
    from: Option<&str>,
) -> Option<String> {
    let what = match (songs, from) {
        ([song], None) => format!("\"{}\"", song.title),
        (_, Some(collection)) => format!("{} songs from \"{collection}\"", songs.len()),
        (_, None) => format!("{} songs", songs.len()),
    };
    match placement {
        Placement::PlayFrom(_) => None,
        Placement::Next => Some(format!("{what} will play next")),
        Placement::Last => Some(format!("Added {what} to the queue")),
    }
}

pub fn play_songs(
    window: &AppWindow,
    deps: &DiscoverDeps,
    songs: Vec<MusicSong>,
    placement: Placement,
) {
    let message = queued_message(placement, &songs, None);
    send_resolved(window, deps, songs, placement, message);
}

pub fn send_resolved(
    window: &AppWindow,
    deps: &DiscoverDeps,
    songs: Vec<MusicSong>,
    placement: Placement,
    message: Option<String>,
) {
    if songs.is_empty() {
        show_status(window, "Nothing to play here");
        return;
    }
    let request = matches!(placement, Placement::PlayFrom(_))
        .then(|| deps.latest_play.fetch_add(1, Ordering::Relaxed) + 1);
    let deps = deps.clone();
    let weak_window = window.as_weak();
    deps.ctx.rt.clone().spawn(async move {
        let tracks = resolve_tracks(&deps.matcher, deps.ctx.session.get(), &songs).await;
        let superseded =
            request.is_some_and(|request| deps.latest_play.load(Ordering::Relaxed) != request);
        if superseded {
            return;
        }
        deps.ctx.send(command_for(placement, tracks));
        if let Some(message) = message {
            on_ui(&weak_window, move |window| show_status(window, &message));
        }
    });
}

pub fn play_collection(
    window: &AppWindow,
    deps: &DiscoverDeps,
    browse_id: String,
    placement: Placement,
) {
    let source = Arc::clone(&deps.source);
    let settings = deps.settings();
    let collection_deps = deps.clone();
    let weak_window = window.as_weak();
    deps.ctx.rt.spawn(async move {
        let result = source.collection(browse_id, settings).await;
        on_ui(&weak_window, move |window| match result {
            Ok(page) => {
                let message = queued_message(placement, &page.songs, Some(&page.title));
                send_resolved(window, &collection_deps, page.songs, placement, message);
            }
            Err(error) => show_status(
                window,
                &format!("Could not load it from YouTube Music: {error}"),
            ),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str) -> MusicSong {
        MusicSong {
            video_id: "abc".into(),
            title: title.into(),
            artists: Vec::new(),
            album: None,
            duration_ms: None,
            thumbnail_url: None,
        }
    }

    #[test]
    fn queue_messages_name_what_was_added() {
        assert_eq!(
            queued_message(Placement::PlayFrom(0), &[song("A")], None),
            None
        );
        assert_eq!(
            queued_message(Placement::Next, &[song("A")], None).as_deref(),
            Some("\"A\" will play next")
        );
        assert_eq!(
            queued_message(Placement::Last, &[song("A"), song("B")], Some("Mix")).as_deref(),
            Some("Added 2 songs from \"Mix\" to the queue")
        );
    }
}
