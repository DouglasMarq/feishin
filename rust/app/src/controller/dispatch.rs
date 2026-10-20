use super::{Command, Controller};

impl Controller {
    pub fn handle(&mut self, command: Command) -> bool {
        self.dispatch(command);
        self.finish_shutdown_once_drained();
        !self.shutdown.is_finished()
    }

    fn dispatch(&mut self, command: Command) {
        match command {
            Command::Player(event) => self.on_player_event(event),
            Command::Media(event) => self.on_media_event(event),
            command @ (Command::Connected { .. } | Command::RemoveServer(_)) => {
                self.dispatch_server_command(command);
            }
            command @ (Command::PlayFromList { .. }
            | Command::EnqueueFromList { .. }
            | Command::ListStream(_)) => self.dispatch_list_command(command),
            Command::TogglePause => self.toggle_pause(),
            Command::Next => self.next(),
            Command::Previous => self.previous(),
            Command::Seek(position_ms) => self.seek(position_ms),
            Command::SetVolume(volume) => self.set_volume(volume),
            Command::ToggleMute => self.toggle_mute(),
            Command::ToggleShuffle => self.toggle_shuffle(),
            Command::CycleRepeat => self.cycle_repeat(),
            Command::PlayUid(uid) => self.play_uid(uid),
            Command::PlaySongs { songs, song_id } => self.play_songs(songs, song_id),
            Command::AddNext(songs) => self.add_next(songs),
            Command::AddLast(songs) => self.add_last(songs),
            Command::Remove(uids) => self.remove(&uids),
            Command::Move { uids, before } => self.move_items(&uids, before),
            Command::SleepTimer(mode) => self.set_sleep_timer(mode),
            Command::SleepTick { elapsed_ms } => self.sleep_tick(elapsed_ms),
            Command::RestoreQueue { request, queue } => self.restore_queue(request, queue),
            Command::Persistence(event) => self.on_persistence_event(event),
            Command::Shutdown { ui, done } => self.request_shutdown(ui, done),
            Command::GetSettings(reply) => self.reply_with_settings(reply),
            Command::ApplyPlayback(playback) => self.apply_playback(playback),
            Command::SaveIntegrations(integrations) => self.save_integrations(integrations),
            Command::SaveInterface {
                queue_panel_open,
                follow_current_song,
            } => self.save_interface(queue_panel_open, follow_current_song),
            Command::SaveLanguage(language) => self.save_language(language),
            Command::SaveAppearance(appearance) => self.save_appearance(appearance),
            command => self.dispatch_source_command(command),
        }
    }

    fn dispatch_server_command(&mut self, command: Command) {
        match command {
            Command::Connected {
                config,
                credential,
                others,
            } => self.connected(config, credential, others),
            Command::RemoveServer(server_id) => self.remove_server(&server_id),
            _ => tracing::debug!("a command reached the wrong handler"),
        }
    }

    fn dispatch_source_command(&mut self, command: Command) {
        match command {
            Command::Radio(command) => self.on_radio_command(command),
            Command::Tracks(command) => self.on_track_command(command),
            Command::YouTube(command) => self.on_youtube_command(command),
            Command::Remote(command) => self.on_remote_command(command),
            Command::MediaCoverCached { song_id } => self.on_media_cover_cached(&song_id),
            _ => tracing::debug!("a command reached the wrong handler"),
        }
    }
}
