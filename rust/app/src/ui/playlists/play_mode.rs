use feishin_core::model::Song;
use slint::ComponentHandle;

use crate::controller::Command;
use crate::ui::{AppWindow, Context, PlaylistPlayMode, on_ui, show_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Now,
    Next,
    Last,
}

impl From<PlaylistPlayMode> for PlayMode {
    fn from(mode: PlaylistPlayMode) -> Self {
        match mode {
            PlaylistPlayMode::Now => Self::Now,
            PlaylistPlayMode::Next => Self::Next,
            PlaylistPlayMode::Last => Self::Last,
        }
    }
}

impl PlayMode {
    pub fn command(self, songs: Vec<Song>) -> Command {
        match self {
            Self::Now => Command::PlaySongs {
                songs,
                song_id: None,
            },
            Self::Next => Command::AddNext(songs),
            Self::Last => Command::AddLast(songs),
        }
    }
}

pub fn play(ctx: &Context, window: &AppWindow, id: &str, mode: PlayMode) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let (id, task_ctx, weak) = (id.to_owned(), ctx.clone(), window.as_weak());
    ctx.rt.spawn(async move {
        match server.playlist_songs(&id).await {
            Ok(songs) if songs.is_empty() => on_ui(&weak, |window| {
                show_status(window, "This playlist is empty");
            }),
            Ok(songs) => task_ctx.send(mode.command(songs)),
            Err(error) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not load the playlist: {error}"));
            }),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_modes_map_to_commands() {
        assert_eq!(PlayMode::from(PlaylistPlayMode::Next), PlayMode::Next);
        assert_eq!(PlayMode::from(PlaylistPlayMode::Last), PlayMode::Last);
        assert_eq!(PlayMode::from(PlaylistPlayMode::Now), PlayMode::Now);
        assert!(matches!(
            PlayMode::Now.command(Vec::new()),
            Command::PlaySongs { song_id: None, .. }
        ));
        assert!(matches!(
            PlayMode::Last.command(Vec::new()),
            Command::AddLast(_)
        ));
    }
}
