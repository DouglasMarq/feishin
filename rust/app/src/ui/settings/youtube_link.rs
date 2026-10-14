use std::sync::Arc;

use feishin_core::queue::QueueTrack;
use feishin_youtube::{YouTubeError, video_id_from_link};
use slint::ComponentHandle;

use super::view::Shared;
use super::youtube::youtube_form;
use crate::controller::{Command, TrackCommand};
use crate::ui::{AppWindow, Context, SettingsState, on_ui, show_status};
use crate::youtube::{queue_track_of, yt_dlp_for};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkPlacement {
    PlayNow,
    Next,
    Last,
}

fn command_for(placement: LinkPlacement, track: QueueTrack) -> Command {
    Command::Tracks(match placement {
        LinkPlacement::PlayNow => TrackCommand::PlayNow {
            tracks: vec![track],
            start_index: Some(0),
        },
        LinkPlacement::Next => TrackCommand::AddNext(vec![track]),
        LinkPlacement::Last => TrackCommand::AddLast(vec![track]),
    })
}

fn added_message(placement: LinkPlacement, title: &str) -> Option<String> {
    match placement {
        LinkPlacement::PlayNow => None,
        LinkPlacement::Next => Some(tr!("\"{}\" plays next", title)),
        LinkPlacement::Last => Some(tr!("Added \"{}\" to the queue", title)),
    }
}

fn enqueue_link(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>, placement: LinkPlacement) {
    let state = window.global::<SettingsState>();
    let Some(video_id) = video_id_from_link(&state.get_youtube_link()) else {
        show_status(window, &tr!("Paste a YouTube or YouTube Music link first"));
        return;
    };
    let yt_dlp = yt_dlp_for(&youtube_form(&state, &shared.view()));
    show_status(window, &tr!("Looking up the YouTube link..."));
    let weak_window = window.as_weak();
    let commands_ctx = ctx.clone();
    ctx.rt.spawn(async move {
        let message = match yt_dlp.video(&video_id).await {
            Ok(video) => {
                commands_ctx.send(command_for(placement, queue_track_of(&video)));
                added_message(placement, &video.title)
            }
            Err(error) => Some(link_failure_message(&error)),
        };
        if let Some(message) = message {
            on_ui(&weak_window, move |window| show_status(window, &message));
        }
    });
}

fn link_failure_message(error: &YouTubeError) -> String {
    match error {
        YouTubeError::NotInstalled => tr!("Install yt-dlp to play YouTube tracks"),
        other => tr!("Could not play from YouTube: {}", other),
    }
}

pub(super) fn play_link(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    enqueue_link(window, ctx, shared, LinkPlacement::PlayNow);
}

pub(super) fn play_link_next(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    enqueue_link(window, ctx, shared, LinkPlacement::Next);
}

pub(super) fn add_link(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    enqueue_link(window, ctx, shared, LinkPlacement::Last);
}
