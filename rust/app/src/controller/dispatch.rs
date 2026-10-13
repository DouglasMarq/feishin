use super::{Command, Controller};

impl Controller {
    pub fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Shutdown { ui, done } => {
                self.shut_down(ui, done);
                false
            }
            running_command => {
                self.handle_running(running_command);
                true
            }
        }
    }

    fn handle_running(&mut self, command: Command) {
        match command {
            Command::Player(event) => self.on_player_event(event),
            Command::Media(event) => self.on_media_event(event),
            Command::Connected { config, others } => self.connected(config, others),
            Command::RestoreQueue { request, queue } => self.restore_queue(request, queue),
            Command::SleepTimer(mode) => self.set_sleep_timer(mode),
            Command::SleepTick { elapsed_ms } => self.sleep_tick(elapsed_ms),
            Command::GetSettings(reply) => self.reply_with_settings(reply),
            Command::ApplyPlayback(playback) => self.apply_playback(playback),
            Command::SaveIntegrations(integrations) => self.save_integrations(integrations),
            Command::SaveInterface {
                queue_panel_open,
                follow_current_song,
            } => self.save_interface(queue_panel_open, follow_current_song),
            Command::RemoveServer(server_id) => self.remove_server(&server_id),
            transport_or_queue_command => self.handle_transport(transport_or_queue_command),
        }
    }

    fn handle_transport(&mut self, command: Command) {
        match command {
            Command::TogglePause => self.toggle_pause(),
            Command::Next => self.next(),
            Command::Previous => self.previous(),
            Command::Seek(position_ms) => self.seek(position_ms),
            Command::SetVolume(volume) => self.set_volume(volume),
            Command::ToggleMute => self.toggle_mute(),
            queue_command => self.handle_queue_command(queue_command),
        }
    }

    fn handle_queue_command(&mut self, command: Command) {
        match command {
            Command::PlayFromList { query, song_id } => self.play_from_list(query, song_id),
            Command::PlayNow {
                request,
                songs,
                song_id,
            } if request == self.play_request => self.play_now(songs, &song_id),
            Command::PlaySongs { songs, song_id } => self.play_songs(songs, song_id),
            Command::PlayUid(uid) => self.play_uid(uid),
            Command::AddNext(songs) => self.add_next(songs),
            Command::AddLast(songs) => self.add_last(songs),
            Command::Remove(uids) => self.remove(&uids),
            Command::Move { uids, before } => self.move_items(&uids, before),
            Command::ToggleShuffle => self.toggle_shuffle(),
            Command::CycleRepeat => self.cycle_repeat(),
            _stale_or_routed_elsewhere => {}
        }
    }
}
