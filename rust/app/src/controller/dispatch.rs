use super::{Command, Controller};

impl Controller {
    pub fn handle(&mut self, command: Command) -> bool {
        let keeps_running = !matches!(command, Command::Shutdown { .. });
        self.dispatch(command);
        keeps_running
    }

    fn dispatch(&mut self, command: Command) {
        match command {
            Command::Player(event) => self.on_player_event(event),
            Command::Media(event) => self.on_media_event(event),
            Command::Connected { config, others } => self.connected(config, others),
            Command::PlayFromList { query, song_id } => self.play_from_list(query, song_id),
            Command::PlayNow {
                request,
                songs,
                song_id,
            } => self.play_requested(request, songs, &song_id),
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
            Command::Shutdown { ui, done } => self.shut_down(ui, done),
            Command::GetSettings(reply) => self.reply_with_settings(reply),
            Command::ApplyPlayback(playback) => self.apply_playback(playback),
            Command::SaveIntegrations(integrations) => self.save_integrations(integrations),
            Command::SaveInterface {
                queue_panel_open,
                follow_current_song,
            } => self.save_interface(queue_panel_open, follow_current_song),
            Command::RemoveServer(server_id) => self.remove_server(&server_id),
        }
    }
}
