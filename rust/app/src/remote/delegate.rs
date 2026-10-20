use std::time::Duration;

use async_trait::async_trait;
use feishin_remote::{
    DeviceInfo, HostDelegate, RemoteControl, RemoteFailure, ServerKey, TrackPlacement,
    TransferPayload,
};
use tokio::sync::{mpsc, oneshot};

use super::approval::Approvals;
use super::lists::{placement_of, song_source_of};
use super::transfer::{LocalServer, adapt_payload, adapt_tracks};
use crate::controller::{Command, RemoteCommand, TrackCommand, forward_command};
use crate::list_stream::ListRow;
use crate::session::Session;
use crate::ui::AppWindow;

const HAND_OVER_TIMEOUT: Duration = Duration::from_secs(5);

pub struct AppHostDelegate {
    pub commands: mpsc::UnboundedSender<Command>,
    pub session: Session,
    pub approvals: Approvals,
    pub window: slint::Weak<AppWindow>,
}

impl AppHostDelegate {
    fn local_server(&self) -> Option<LocalServer> {
        self.session
            .get()
            .map(|server| LocalServer::of(server.as_ref()))
    }
}

#[async_trait]
impl HostDelegate for AppHostDelegate {
    async fn approve(&self, device: DeviceInfo) -> bool {
        self.approvals.ask(&self.window, &device).await
    }

    fn control(&self, device: &DeviceInfo, control: RemoteControl) -> Result<(), RemoteFailure> {
        let command = command_of(control, self.local_server().as_ref())?;
        tracing::debug!("{} sent a playback command", device.name);
        forward_command(&self.commands, command);
        Ok(())
    }

    async fn adopt(&self, payload: TransferPayload) -> Result<(), RemoteFailure> {
        let adopted = adapt_payload(payload, self.local_server().as_ref())?;
        let command = Command::Remote(RemoteCommand::AdoptTransfer(Box::new(adopted)));
        forward_command(&self.commands, command);
        Ok(())
    }

    async fn hand_over(&self) -> Result<TransferPayload, RemoteFailure> {
        let (reply, replied) = oneshot::channel();
        forward_command(
            &self.commands,
            Command::Remote(RemoteCommand::TakeTransfer(reply)),
        );
        tokio::time::timeout(HAND_OVER_TIMEOUT, replied)
            .await
            .map_err(|_| RemoteFailure::Busy)?
            .map_err(|_| RemoteFailure::Stopped)?
    }

    fn server(&self) -> Option<ServerKey> {
        self.local_server().map(|local| local.key)
    }
}

pub fn command_of(
    control: RemoteControl,
    local: Option<&LocalServer>,
) -> Result<Command, RemoteFailure> {
    match playback_command(&control) {
        Some(command) => Ok(command),
        None => queue_command(control, local),
    }
}

fn playback_command(control: &RemoteControl) -> Option<Command> {
    Some(match control {
        RemoteControl::TogglePause => Command::TogglePause,
        RemoteControl::Next => Command::Next,
        RemoteControl::Previous => Command::Previous,
        RemoteControl::Seek { position_ms } => Command::Seek(*position_ms),
        RemoteControl::SetVolume { volume } => Command::SetVolume(*volume),
        RemoteControl::ToggleMute => Command::ToggleMute,
        RemoteControl::ToggleShuffle => Command::ToggleShuffle,
        RemoteControl::CycleRepeat => Command::CycleRepeat,
        RemoteControl::PlayUid { uid } => Command::PlayUid(*uid),
        RemoteControl::Remove { uids } => Command::Remove(uids.clone()),
        RemoteControl::Move { uids, before } => Command::Move {
            uids: uids.clone(),
            before: *before,
        },
        _ => return None,
    })
}

fn queue_command(
    control: RemoteControl,
    local: Option<&LocalServer>,
) -> Result<Command, RemoteFailure> {
    match control {
        RemoteControl::PlayTracks {
            tracks,
            start_index,
            server,
        } => {
            let adapted = adapt_tracks(tracks, start_index, server.as_ref(), local)?;
            Ok(Command::Tracks(TrackCommand::PlayNow {
                tracks: adapted.tracks,
                start_index: adapted.start_index,
            }))
        }
        RemoteControl::AddTracks {
            tracks,
            placement,
            server,
        } => {
            let tracks = adapt_tracks(tracks, None, server.as_ref(), local)?.tracks;
            Ok(Command::Tracks(match placement {
                TrackPlacement::Next => TrackCommand::AddNext(tracks),
                TrackPlacement::Last => TrackCommand::AddLast(tracks),
            }))
        }
        other => list_command(other, local),
    }
}

fn list_command(
    control: RemoteControl,
    local: Option<&LocalServer>,
) -> Result<Command, RemoteFailure> {
    match control {
        RemoteControl::PlayList {
            list,
            song_id,
            row,
            server,
        } => {
            same_server(&server, local)?;
            Ok(Command::PlayFromList {
                source: song_source_of(list),
                song_id,
                row_index: ListRow(row),
            })
        }
        RemoteControl::EnqueueList {
            list,
            ranges,
            placement,
            server,
        } => {
            same_server(&server, local)?;
            Ok(Command::EnqueueFromList {
                source: song_source_of(list),
                ranges,
                placement: placement_of(placement),
            })
        }
        other => playback_command(&other).ok_or(RemoteFailure::Stopped),
    }
}

fn same_server(server: &ServerKey, local: Option<&LocalServer>) -> Result<(), RemoteFailure> {
    match local {
        None => Err(RemoteFailure::NotConnected),
        Some(local) if local.key == *server => Ok(()),
        Some(_) => Err(RemoteFailure::DifferentServer),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use feishin_remote::RemoteList;

    use super::*;

    fn local() -> LocalServer {
        LocalServer {
            key: ServerKey {
                url: "http://nas".into(),
                username: "me".into(),
            },
            id: Arc::from("local"),
        }
    }

    fn play_list(url: &str) -> RemoteControl {
        RemoteControl::PlayList {
            list: RemoteList::Playlist { id: "p".into() },
            song_id: None,
            row: 2,
            server: ServerKey {
                url: url.into(),
                username: "me".into(),
            },
        }
    }

    #[test]
    fn lists_play_only_from_the_same_server() {
        assert!(matches!(
            command_of(play_list("http://nas"), Some(&local())),
            Ok(Command::PlayFromList {
                row_index: ListRow(2),
                ..
            })
        ));
        assert_eq!(
            command_of(play_list("http://other"), Some(&local())).err(),
            Some(RemoteFailure::DifferentServer)
        );
        assert_eq!(
            command_of(play_list("http://nas"), None).err(),
            Some(RemoteFailure::NotConnected)
        );
    }

    #[test]
    fn simple_controls_map_to_their_commands() {
        assert!(matches!(
            command_of(RemoteControl::Seek { position_ms: 9 }, None),
            Ok(Command::Seek(9))
        ));
        assert!(matches!(
            command_of(
                RemoteControl::Move {
                    uids: vec![1],
                    before: Some(3)
                },
                None
            ),
            Ok(Command::Move {
                before: Some(3),
                ..
            })
        ));
    }
}
