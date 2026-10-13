use feishin_core::model::Song;
use slint::ComponentHandle;

use super::results::with_results;
use super::searches::Searches;
use crate::controller::Command;
use crate::ui::{AppWindow, Context, SearchState};

type SongCommand = fn(&Searches, Song) -> Command;

fn song_command(window: &AppWindow, index: i32, command: SongCommand) -> Option<Command> {
    with_results(window, |results| {
        let searches = results.searches.borrow();
        let song = searches.song(index)?.clone();
        Some(command(&searches, song))
    })
    .flatten()
}

fn song_handler(window: &AppWindow, ctx: &Context, command: SongCommand) -> impl Fn(i32) + 'static {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    move |index: i32| {
        let command = weak
            .upgrade()
            .and_then(|window| song_command(&window, index, command));
        if let Some(command) = command {
            ctx.send(command);
        }
    }
}

fn play_from_results(searches: &Searches, song: Song) -> Command {
    Command::PlaySongs {
        songs: searches.songs.items.clone(),
        song_id: Some(song.id),
    }
}

fn play_only(_: &Searches, song: Song) -> Command {
    Command::PlaySongs {
        songs: vec![song],
        song_id: None,
    }
}

fn play_next(_: &Searches, song: Song) -> Command {
    Command::AddNext(vec![song])
}

fn add_to_queue(_: &Searches, song: Song) -> Command {
    Command::AddLast(vec![song])
}

pub fn install_song_commands(window: &AppWindow, ctx: &Context) {
    let state = window.global::<SearchState>();
    state.on_play(song_handler(window, ctx, play_from_results));
    state.on_play_only(song_handler(window, ctx, play_only));
    state.on_play_next(song_handler(window, ctx, play_next));
    state.on_add_to_queue(song_handler(window, ctx, add_to_queue));
}
