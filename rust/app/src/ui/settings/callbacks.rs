use std::sync::Arc;

use slint::ComponentHandle;

use super::display::set_lyrics_rows;
use super::loading::{load, save_integrations};
use super::lyrics_sources::move_lyrics_source;
use super::servers::{log_out, remove_server, switch_server};
use super::view::{PlaybackForm, Shared};
use crate::controller::Command;
use crate::ui::{AppState, AppWindow, Context, SettingsState, login};

type WindowAction = fn(&AppWindow, &Context, &Arc<Shared>);

#[derive(Clone)]
struct Binding {
    weak_window: slint::Weak<AppWindow>,
    ctx: Context,
    shared: Arc<Shared>,
}

impl Binding {
    fn new(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) -> Self {
        Self {
            weak_window: window.as_weak(),
            ctx: ctx.clone(),
            shared: Arc::clone(shared),
        }
    }

    fn run(&self, action: impl FnOnce(&AppWindow, &Context, &Arc<Shared>)) {
        if let Some(window) = self.weak_window.upgrade() {
            action(&window, &self.ctx, &self.shared);
        }
    }

    fn action(&self, action: WindowAction) -> impl Fn() + 'static {
        let binding = self.clone();
        move || binding.run(action)
    }
}

fn apply_playback(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();
    let form = PlaybackForm {
        replay_gain: state.get_replay_gain(),
        gapless: state.get_gapless(),
        device: state.get_device_index(),
        bitrate: state.get_bitrate_index(),
        format: state.get_format_index(),
        scrobble: state.get_scrobble(),
    };
    let mut view = shared.view();
    let playback = view.playback(form);
    view.settings.playback = playback.clone();
    ctx.send(Command::ApplyPlayback(playback));
}

fn apply_integrations(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();
    let mut view = shared.view();
    view.settings.integrations.mpris = state.get_mpris();
    view.settings.integrations.discord_rich_presence = state.get_discord();
    save_integrations(ctx, &mut view);
}

fn toggle_lyrics_source(window: &AppWindow, ctx: &Context, shared: &Shared, row: i32) {
    let mut view = shared.view();
    let Some(source) = usize::try_from(row)
        .ok()
        .and_then(|position| view.lyrics.get_mut(position))
    else {
        return;
    };
    source.enabled = !source.enabled;
    set_lyrics_rows(&window.global::<SettingsState>(), &view.lyrics);
    save_integrations(ctx, &mut view);
}

fn reorder_lyrics_source(window: &AppWindow, ctx: &Context, shared: &Shared, row: i32, delta: i32) {
    let mut view = shared.view();
    let (Ok(row), Ok(delta)) = (usize::try_from(row), isize::try_from(delta)) else {
        return;
    };
    if move_lyrics_source(&mut view.lyrics, row, delta) {
        set_lyrics_rows(&window.global::<SettingsState>(), &view.lyrics);
        save_integrations(ctx, &mut view);
    }
}

fn apply_interface(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let follow_current_song = window.global::<SettingsState>().get_follow_current_song();
    let queue_panel_open = window.global::<AppState>().get_queue_open();
    let mut view = shared.view();
    view.settings.ui.follow_current_song = follow_current_song;
    view.settings.ui.queue_panel_open = queue_panel_open;
    ctx.send(Command::SaveInterface {
        queue_panel_open,
        follow_current_song,
    });
}

fn install_lyrics_source_callbacks(state: &SettingsState, binding: &Binding) {
    let toggle_binding = binding.clone();
    state.on_toggle_lyrics_source(move |row| {
        toggle_binding.run(|window, ctx, shared| toggle_lyrics_source(window, ctx, shared, row));
    });
    let reorder_binding = binding.clone();
    state.on_move_lyrics_source(move |row, delta| {
        reorder_binding
            .run(|window, ctx, shared| reorder_lyrics_source(window, ctx, shared, row, delta));
    });
}

fn install_server_callbacks(state: &SettingsState, binding: &Binding) {
    let switch_binding = binding.clone();
    state.on_switch_server(move |id| {
        switch_binding.run(|window, ctx, shared| switch_server(window, ctx, shared, id));
    });
    let remove_binding = binding.clone();
    state.on_remove_server(move |id| {
        remove_binding.run(|window, ctx, shared| remove_server(window, ctx, shared, &id));
    });
    state.on_add_server(binding.action(|window, _, _| login::show_login(window, "")));
    state.on_log_out(binding.action(|window, ctx, shared| log_out(window, ctx, shared)));
}

pub(super) fn install_with(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();
    let binding = Binding::new(window, ctx, shared);
    let reload = binding.action(load);
    state.on_load(move |_| reload());
    state.on_playback_changed(binding.action(apply_playback));
    state.on_integrations_changed(binding.action(apply_integrations));
    install_lyrics_source_callbacks(&state, &binding);
    state.on_interface_changed(binding.action(apply_interface));
    install_server_callbacks(&state, &binding);
    load(window, ctx, shared);
}
