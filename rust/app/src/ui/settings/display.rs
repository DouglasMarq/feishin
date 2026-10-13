use feishin_storage::settings::Settings;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::choices::replay_gain_index;
use super::lyrics_sources::{LyricsSource, lyrics_source_name};
use super::view::{Shared, View};
use crate::controller::SettingsSnapshot;
use crate::ui::{AppWindow, LyricsSourceRow, ServerRow, SettingsState};

pub(super) fn server_rows(settings: &Settings) -> Vec<ServerRow> {
    settings
        .servers
        .iter()
        .map(|server| ServerRow {
            id: server.id.as_str().into(),
            name: server.name.as_str().into(),
            kind: format!("{:?}", server.server_type).into(),
            detail: format!("{} at {}", server.username, server.url).into(),
            current: settings.current_server_id.as_deref() == Some(server.id.as_str()),
        })
        .collect()
}

fn string_model(values: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values
            .iter()
            .map(|value| SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    ))
}

pub(super) fn slint_index(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(0)
}

pub(super) fn set_lyrics_rows(state: &SettingsState, sources: &[LyricsSource]) {
    let rows: Vec<LyricsSourceRow> = sources
        .iter()
        .map(|source| LyricsSourceRow {
            id: source.id.as_str().into(),
            name: lyrics_source_name(&source.id).into(),
            enabled: source.enabled,
        })
        .collect();
    state.set_lyrics_sources(ModelRc::new(VecModel::from(rows)));
}

fn set_playback_fields(state: &SettingsState, view: &View) {
    let playback = &view.settings.playback;
    state.set_replay_gain(replay_gain_index(playback.replay_gain));
    state.set_gapless(playback.gapless);
    state.set_scrobble(playback.scrobble);
    state.set_devices(string_model(&view.devices.labels));
    state.set_device_index(slint_index(view.devices.selected));
    state.set_bitrates(string_model(&view.bitrates.labels));
    state.set_bitrate_index(slint_index(view.bitrates.selected));
    state.set_formats(string_model(&view.formats.labels));
    state.set_format_index(slint_index(view.formats.selected));
}

pub(super) fn show(window: &AppWindow, shared: &Shared, snapshot: SettingsSnapshot) {
    let state = window.global::<SettingsState>();
    let view = View::new(snapshot);
    let settings = &view.settings;
    set_playback_fields(&state, &view);
    state.set_mpris(settings.integrations.mpris);
    state.set_discord(settings.integrations.discord_rich_presence);
    set_lyrics_rows(&state, &view.lyrics);
    state.set_servers(ModelRc::new(VecModel::from(server_rows(settings))));
    state.set_follow_current_song(settings.ui.follow_current_song);
    state.set_switching("".into());
    state.set_loaded(true);
    *shared.view() = view;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::test_support::server;

    #[test]
    fn servers_mark_the_current_one() {
        let settings = Settings {
            servers: vec![server("a"), server("b")],
            current_server_id: Some("b".into()),
            ..Settings::default()
        };
        let rows = server_rows(&settings);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].current && rows[1].current);
        assert_eq!(rows[0].kind, "Navidrome");
        assert_eq!(rows[0].detail, "me at https://a.example.com");
    }
}
