use feishin_core::model::ServerType;
use feishin_storage::settings::{MAX_CROSSFADE_SECONDS, Settings};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::choices::replay_gain_choice;
use super::language::show_languages;
use super::lyrics_sources::LyricsSource;
use super::view::{Shared, View};
use super::youtube::show_youtube;
use crate::controller::SettingsSnapshot;
use crate::lyrics_provider::lyrics_source_label;
use crate::ui::{AppWindow, LyricsSourceRow, ServerRow, SettingsState, slint_index};

pub(super) fn server_rows(settings: &Settings) -> Vec<ServerRow> {
    settings
        .servers
        .iter()
        .map(|server| ServerRow {
            id: server.id.as_str().into(),
            name: server.name.as_str().into(),
            kind: server_kind_label(server.server_type).into(),
            detail: tr!("{} at {}", server.username, server.url).into(),
            is_current: settings.current_server_id.as_deref() == Some(server.id.as_str()),
        })
        .collect()
}

fn server_kind_label(server_type: ServerType) -> &'static str {
    match server_type {
        ServerType::Navidrome => "Navidrome",
        ServerType::Subsonic => "Subsonic",
        ServerType::Jellyfin => "Jellyfin",
    }
}

pub(super) fn string_model(values: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values
            .iter()
            .map(|value| SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    ))
}

pub(super) fn set_lyrics_rows(state: &SettingsState, sources: &[LyricsSource]) {
    let rows: Vec<LyricsSourceRow> = sources
        .iter()
        .map(|source| LyricsSourceRow {
            id: source.source.name().into(),
            name: lyrics_source_label(source.source).into(),
            is_enabled: source.is_enabled,
        })
        .collect();
    state.set_lyrics_sources(ModelRc::new(VecModel::from(rows)));
}

fn set_playback_fields(state: &SettingsState, view: &View) {
    let playback = &view.settings.playback;
    state.set_replay_gain(replay_gain_choice(playback.replay_gain));
    state.set_is_gapless(playback.is_gapless);
    state.set_max_crossfade_seconds(i32::from(MAX_CROSSFADE_SECONDS));
    state.set_crossfade_seconds(i32::from(
        playback.crossfade_seconds.min(MAX_CROSSFADE_SECONDS),
    ));
    state.set_is_scrobbling_enabled(playback.is_scrobbling_enabled);
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
    state.set_is_mpris_enabled(settings.integrations.is_mpris_enabled);
    state.set_is_discord_enabled(settings.integrations.is_discord_rich_presence_enabled);
    state.set_is_remote_control_enabled(settings.integrations.is_remote_control_enabled);
    set_lyrics_rows(&state, &view.lyrics);
    show_youtube(&state, &view);
    state.set_servers(ModelRc::new(VecModel::from(server_rows(settings))));
    state.set_is_following_current_song(settings.ui.is_following_current_song);
    show_languages(&state, settings.ui.language.as_deref());
    state.set_switching("".into());
    state.set_is_loaded(true);
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
        assert!(!rows[0].is_current && rows[1].is_current);
        assert_eq!(rows[0].kind, "Navidrome");
        assert_eq!(rows[0].detail, "me at https://a.example.com");
    }

    #[test]
    fn given_each_server_type_the_kind_label_is_its_product_name() {
        assert_eq!(server_kind_label(ServerType::Navidrome), "Navidrome");
        assert_eq!(server_kind_label(ServerType::Subsonic), "Subsonic");
        assert_eq!(server_kind_label(ServerType::Jellyfin), "Jellyfin");
    }
}
