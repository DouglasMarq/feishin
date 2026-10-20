use std::sync::Arc;
use std::sync::atomic::Ordering;

use feishin_core::queue::QueueTrack;
use feishin_youtube::music::MusicSong;
use slint::ComponentHandle;

use super::deps::DiscoverDeps;
use super::mixes::mix_page;
use super::send_order::SendTurn;
use crate::controller::{Command, TrackCommand};
use crate::discover::mixes::MixId;
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
    match placement {
        Placement::PlayFrom(_) => None,
        Placement::Next => Some(play_next_message(songs, from)),
        Placement::Last => Some(added_to_queue_message(songs, from)),
    }
}

fn play_next_message(songs: &[MusicSong], from: Option<&str>) -> String {
    match (songs, from) {
        ([song], None) => tr!("\"{}\" will play next", song.title),
        (_, Some(collection)) => tr_plural!(
            "{n} song from \"{}\" will play next",
            "{n} songs from \"{}\" will play next",
            songs.len(),
            collection
        ),
        (_, None) => tr_plural!(
            "{n} song will play next",
            "{n} songs will play next",
            songs.len()
        ),
    }
}

fn added_to_queue_message(songs: &[MusicSong], from: Option<&str>) -> String {
    match (songs, from) {
        ([song], None) => tr!("Added \"{}\" to the queue", song.title),
        (_, Some(collection)) => tr_plural!(
            "Added {n} song from \"{}\" to the queue",
            "Added {n} songs from \"{}\" to the queue",
            songs.len(),
            collection
        ),
        (_, None) => tr_plural!(
            "Added {n} song to the queue",
            "Added {n} songs to the queue",
            songs.len()
        ),
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

struct SendTicket {
    turn: SendTurn,
    play_request: Option<u64>,
}

fn ticket_for(deps: &DiscoverDeps, placement: Placement) -> SendTicket {
    SendTicket {
        turn: deps.send_order.take_turn(),
        play_request: matches!(placement, Placement::PlayFrom(_))
            .then(|| deps.latest_play.fetch_add(1, Ordering::Relaxed) + 1),
    }
}

pub fn send_resolved(
    window: &AppWindow,
    deps: &DiscoverDeps,
    songs: Vec<MusicSong>,
    placement: Placement,
    message: Option<String>,
) {
    let ticket = ticket_for(deps, placement);
    send_with_ticket(window, deps, songs, (placement, message), ticket);
}

fn send_with_ticket(
    window: &AppWindow,
    deps: &DiscoverDeps,
    songs: Vec<MusicSong>,
    (placement, message): (Placement, Option<String>),
    mut ticket: SendTicket,
) {
    if songs.is_empty() {
        show_status(window, &tr!("Nothing to play here"));
        return;
    }
    let deps = deps.clone();
    let weak_window = window.as_weak();
    deps.context.runtime.clone().spawn(async move {
        let tracks = resolve_tracks(&deps.matcher, deps.context.session.get(), &songs).await;
        ticket.turn.wait_for_earlier_turns().await;
        let superseded = ticket
            .play_request
            .is_some_and(|request| deps.latest_play.load(Ordering::Relaxed) != request);
        if superseded {
            return;
        }
        deps.context.send(command_for(placement, tracks));
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
    if let Some(mix) = MixId::parse(&browse_id) {
        play_mix(window, deps, mix, placement);
        return;
    }
    let source = Arc::clone(&deps.source);
    let settings = deps.settings();
    let ticket = ticket_for(deps, placement);
    let collection_deps = deps.clone();
    let weak_window = window.as_weak();
    deps.context.runtime.spawn(async move {
        let result = source.collection(browse_id, settings).await;
        on_ui(&weak_window, move |window| match result {
            Ok(page) => {
                let message = queued_message(placement, &page.songs, Some(&page.title));
                send_with_ticket(
                    window,
                    &collection_deps,
                    page.songs,
                    (placement, message),
                    ticket,
                );
            }
            Err(error) => show_status(
                window,
                &tr!("Could not load it from YouTube Music: {}", error),
            ),
        });
    });
}

fn play_mix(window: &AppWindow, deps: &DiscoverDeps, mix: MixId, placement: Placement) {
    let Some(page) = mix_page(window, mix) else {
        show_status(window, &tr!("This mix is no longer available"));
        return;
    };
    let message = queued_message(placement, &page.songs, Some(&page.title));
    send_resolved(window, deps, page.songs, placement, message);
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
