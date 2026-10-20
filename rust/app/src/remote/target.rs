use std::sync::{Arc, RwLock};

use feishin_core::queue::QueueTrack;
use feishin_remote::{ClientMessage, RemoteControl, ServerKey, TrackPlacement};
use tokio::sync::mpsc;

use super::lists::{remote_list_of, remote_placement_of};
use crate::controller::{Command, TrackCommand};

#[derive(Debug, Clone)]
pub struct RemoteTarget {
    pub messages: mpsc::UnboundedSender<ClientMessage>,
}

#[derive(Clone, Default)]
pub struct PlayerTarget(Arc<RwLock<Option<RemoteTarget>>>);

impl PlayerTarget {
    pub fn remote(&self) -> Option<RemoteTarget> {
        self.0.read().ok().and_then(|target| target.clone())
    }

    pub fn is_remote(&self) -> bool {
        self.remote().is_some()
    }

    pub fn set(&self, target: Option<RemoteTarget>) {
        if let Ok(mut current) = self.0.write() {
            *current = target;
        }
    }

    pub fn route(&self, command: Command, local_server: Option<&ServerKey>) -> Option<Command> {
        let Some(target) = self.remote() else {
            return Some(command);
        };
        match remote_control_of(command, local_server) {
            Ok(control) => {
                if target
                    .messages
                    .send(ClientMessage::Control { control })
                    .is_err()
                {
                    tracing::debug!("the remote player link is closed, dropping a command");
                }
                None
            }
            Err(local) => Some(*local),
        }
    }
}

pub fn remote_control_of(
    command: Command,
    local_server: Option<&ServerKey>,
) -> Result<RemoteControl, Box<Command>> {
    let server = local_server.cloned();
    match command {
        Command::TogglePause => Ok(RemoteControl::TogglePause),
        Command::Next => Ok(RemoteControl::Next),
        Command::Previous => Ok(RemoteControl::Previous),
        Command::Seek(position_ms) => Ok(RemoteControl::Seek { position_ms }),
        Command::SetVolume(volume) => Ok(RemoteControl::SetVolume { volume }),
        Command::ToggleMute => Ok(RemoteControl::ToggleMute),
        Command::ToggleShuffle => Ok(RemoteControl::ToggleShuffle),
        Command::CycleRepeat => Ok(RemoteControl::CycleRepeat),
        Command::PlayUid(uid) => Ok(RemoteControl::PlayUid { uid }),
        Command::Remove(uids) => Ok(RemoteControl::Remove { uids }),
        Command::Move { uids, before } => Ok(RemoteControl::Move { uids, before }),
        Command::PlaySongs { songs, song_id } => Ok(RemoteControl::PlayTracks {
            start_index: song_id.and_then(|wanted| songs.iter().position(|song| song.id == wanted)),
            tracks: songs.iter().map(QueueTrack::from).collect(),
            server,
        }),
        Command::AddNext(songs) => Ok(add_tracks(
            songs.iter().map(QueueTrack::from).collect(),
            TrackPlacement::Next,
            server,
        )),
        Command::AddLast(songs) => Ok(add_tracks(
            songs.iter().map(QueueTrack::from).collect(),
            TrackPlacement::Last,
            server,
        )),
        Command::Tracks(tracks) => Ok(track_control(tracks, server)),
        command @ (Command::PlayFromList { .. } | Command::EnqueueFromList { .. }) => {
            list_control(command, server)
        }
        other => Err(Box::new(other)),
    }
}

fn add_tracks(
    tracks: Vec<QueueTrack>,
    placement: TrackPlacement,
    server: Option<ServerKey>,
) -> RemoteControl {
    RemoteControl::AddTracks {
        tracks,
        placement,
        server,
    }
}

fn track_control(command: TrackCommand, server: Option<ServerKey>) -> RemoteControl {
    match command {
        TrackCommand::PlayNow {
            tracks,
            start_index,
        } => RemoteControl::PlayTracks {
            tracks,
            start_index,
            server,
        },
        TrackCommand::AddNext(tracks) => add_tracks(tracks, TrackPlacement::Next, server),
        TrackCommand::AddLast(tracks) => add_tracks(tracks, TrackPlacement::Last, server),
    }
}

fn list_control(
    command: Command,
    server: Option<ServerKey>,
) -> Result<RemoteControl, Box<Command>> {
    let Some(server) = server else {
        return Err(Box::new(command));
    };
    match command {
        Command::PlayFromList {
            source,
            song_id,
            row_index,
        } => Ok(RemoteControl::PlayList {
            list: remote_list_of(source),
            song_id,
            row: row_index.0,
            server,
        }),
        Command::EnqueueFromList {
            source,
            ranges,
            placement,
        } => Ok(RemoteControl::EnqueueList {
            list: remote_list_of(source),
            ranges,
            placement: remote_placement_of(placement),
            server,
        }),
        other => Err(Box::new(other)),
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::Song;

    use super::*;
    use crate::demo::test_song;
    use crate::list_stream::{ListRow, PlaylistId, SongSource};

    fn server() -> ServerKey {
        ServerKey {
            url: "http://navidrome.example".into(),
            username: "me".into(),
        }
    }

    fn songs() -> Vec<Song> {
        vec![test_song("a", 1_000), test_song("b", 1_000)]
    }

    #[test]
    fn playback_and_queue_commands_go_to_the_remote_player() {
        assert_eq!(
            remote_control_of(Command::Seek(5_000), None).ok(),
            Some(RemoteControl::Seek { position_ms: 5_000 })
        );
        let Ok(RemoteControl::PlayTracks {
            tracks,
            start_index,
            server: sent_server,
        }) = remote_control_of(
            Command::PlaySongs {
                songs: songs(),
                song_id: Some("b".into()),
            },
            Some(&server()),
        )
        else {
            panic!("expected PlayTracks");
        };
        assert_eq!(
            (tracks.len(), start_index, sent_server),
            (2, Some(1), Some(server()))
        );
    }

    #[test]
    fn lists_need_a_server_and_everything_else_stays_local() {
        let play_list = || Command::PlayFromList {
            source: SongSource::Playlist(PlaylistId("p".into())),
            song_id: None,
            row_index: ListRow(3),
        };
        assert!(matches!(
            remote_control_of(play_list(), Some(&server())),
            Ok(RemoteControl::PlayList { row: 3, .. })
        ));
        assert!(remote_control_of(play_list(), None).is_err());
        assert!(remote_control_of(Command::SaveLanguage(None), Some(&server())).is_err());
    }

    #[test]
    fn a_local_target_runs_everything_locally() {
        let target = PlayerTarget::default();
        assert!(matches!(
            target.route(Command::Next, None),
            Some(Command::Next)
        ));
        let (messages, mut sent) = mpsc::unbounded_channel();
        target.set(Some(RemoteTarget { messages }));
        assert!(target.route(Command::Next, None).is_none());
        assert_eq!(
            sent.try_recv().ok(),
            Some(ClientMessage::Control {
                control: RemoteControl::Next
            })
        );
        assert!(target.route(Command::SaveLanguage(None), None).is_some());
    }
}
