use feishin_core::queue::QueueTrack;

use super::{Controller, TrackCommand};

impl Controller {
    pub(super) fn on_track_command(&mut self, command: TrackCommand) {
        match command {
            TrackCommand::PlayNow {
                tracks,
                start_index,
            } => self.play_tracks(tracks, start_index),
            TrackCommand::AddNext(tracks) => {
                self.queue.add_tracks_next(tracks);
                self.queue_changed();
            }
            TrackCommand::AddLast(tracks) => {
                self.queue.add_tracks_last(tracks);
                self.queue_changed();
            }
        }
    }

    fn play_tracks(&mut self, tracks: Vec<QueueTrack>, start_index: Option<usize>) {
        self.play_request += 1;
        self.list_streams.cancel_all();
        self.queue.play_tracks_now(tracks, start_index);
        self.publish_queue();
        self.load_current(0);
    }
}
