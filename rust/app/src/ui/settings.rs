use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Result, anyhow};
use feishin_api::MusicServer;
use feishin_core::model::ServerConfig;
use feishin_player::AudioDevice;
use feishin_storage::settings::{PlaybackSettings, ReplayGainSetting, Settings};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokio::sync::oneshot;

use super::{
    AppState, AppWindow, Context, LyricsSourceRow, ServerRow, SettingsState, login, on_ui,
    show_status,
};
use crate::controller::{Command, SettingsSnapshot};

const BITRATES_KBPS: [u32; 7] = [320, 256, 192, 160, 128, 96, 64];
const FORMATS: [&str; 5] = ["mp3", "opus", "aac", "ogg", "flac"];
const LYRICS_SOURCES: [(&str, &str); 4] = [
    ("server", "Server"),
    ("lrclib", "LRCLIB"),
    ("netease", "NetEase"),
    ("genius", "Genius"),
];
/// mpv's name for "follow the system default output".
const AUTO_DEVICE: &str = "auto";

/// Labels shown in a picker and the setting value behind each one.
#[derive(Debug, Default, PartialEq)]
struct Choices<T> {
    labels: Vec<String>,
    values: Vec<T>,
    selected: usize,
}

impl<T: PartialEq> Choices<T> {
    fn new(options: Vec<(String, T)>, current: &T) -> Self {
        let (labels, values): (Vec<_>, Vec<_>) = options.into_iter().unzip();
        let selected = values.iter().position(|v| v == current).unwrap_or(0);
        Self {
            labels,
            values,
            selected,
        }
    }
}

impl<T: Clone> Choices<T> {
    fn value(&self, index: i32) -> Option<T> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.values.get(i))
            .cloned()
    }
}

fn bitrate_choices(current: Option<u32>) -> Choices<Option<u32>> {
    let mut values: Vec<Option<u32>> = std::iter::once(None)
        .chain(BITRATES_KBPS.map(Some))
        .collect();
    if !values.contains(&current) {
        values.push(current);
    }
    let options = values
        .into_iter()
        .map(|value| {
            let label = value.map_or_else(|| "Original".to_owned(), |kbps| format!("{kbps} kbps"));
            (label, value)
        })
        .collect();
    Choices::new(options, &current)
}

fn format_choices(current: Option<&str>) -> Choices<Option<String>> {
    let current = current
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| !f.is_empty());
    let mut values: Vec<Option<String>> = std::iter::once(None)
        .chain(FORMATS.map(|f| Some(f.to_owned())))
        .collect();
    if !values.contains(&current) {
        values.push(current.clone());
    }
    let options = values
        .into_iter()
        .map(|value| (value.clone().unwrap_or_else(|| "Original".into()), value))
        .collect();
    Choices::new(options, &current)
}

fn device_choices(devices: &[AudioDevice], current: Option<&str>) -> Choices<Option<String>> {
    let mut options = Vec::new();
    if !devices.iter().any(|d| d.name == AUTO_DEVICE) {
        options.push(("System default".to_owned(), None));
    }
    for device in devices {
        let label = if device.description.is_empty() {
            device.name.clone()
        } else {
            device.description.clone()
        };
        let value = (device.name != AUTO_DEVICE).then(|| device.name.clone());
        options.push((label, value));
    }
    let current = current
        .filter(|name| *name != AUTO_DEVICE)
        .map(str::to_owned);
    if let Some(name) = &current
        && !options.iter().any(|(_, v)| v.as_ref() == Some(name))
    {
        options.push((format!("{name} (unavailable)"), current.clone()));
    }
    Choices::new(options, &current)
}

fn replay_gain_index(setting: ReplayGainSetting) -> i32 {
    match setting {
        ReplayGainSetting::Off => 0,
        ReplayGainSetting::Track => 1,
        ReplayGainSetting::Album => 2,
    }
}

fn replay_gain_from_index(index: i32) -> ReplayGainSetting {
    match index {
        1 => ReplayGainSetting::Track,
        2 => ReplayGainSetting::Album,
        _ => ReplayGainSetting::Off,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LyricsSource {
    id: String,
    enabled: bool,
}

/// Enabled sources in priority order, then the known ones that are turned off.
fn lyrics_sources(enabled: &[String]) -> Vec<LyricsSource> {
    let mut sources: Vec<LyricsSource> = Vec::new();
    for id in enabled {
        if !sources.iter().any(|s| s.id == *id) {
            sources.push(LyricsSource {
                id: id.clone(),
                enabled: true,
            });
        }
    }
    for (id, _) in LYRICS_SOURCES {
        if !sources.iter().any(|s| s.id == id) {
            sources.push(LyricsSource {
                id: id.into(),
                enabled: false,
            });
        }
    }
    sources
}

fn enabled_lyrics_sources(sources: &[LyricsSource]) -> Vec<String> {
    sources
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.clone())
        .collect()
}

fn move_lyrics_source(sources: &mut [LyricsSource], index: usize, delta: isize) -> bool {
    match index.checked_add_signed(delta) {
        Some(target) if index < sources.len() && target < sources.len() => {
            sources.swap(index, target);
            true
        }
        _ => false,
    }
}

fn lyrics_source_name(id: &str) -> String {
    LYRICS_SOURCES
        .iter()
        .find(|(known, _)| *known == id)
        .map_or_else(|| id.to_owned(), |(_, name)| (*name).to_owned())
}

fn server_rows(settings: &Settings) -> Vec<ServerRow> {
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

/// What the playback section currently shows.
#[derive(Debug, Clone, Copy)]
struct PlaybackForm {
    replay_gain: i32,
    gapless: bool,
    device: i32,
    bitrate: i32,
    format: i32,
    scrobble: bool,
}

#[derive(Default)]
struct View {
    settings: Settings,
    devices: Choices<Option<String>>,
    bitrates: Choices<Option<u32>>,
    formats: Choices<Option<String>>,
    lyrics: Vec<LyricsSource>,
}

impl View {
    fn new(snapshot: SettingsSnapshot) -> Self {
        let SettingsSnapshot { settings, devices } = snapshot;
        let playback = &settings.playback;
        Self {
            devices: device_choices(&devices, playback.audio_device.as_deref()),
            bitrates: bitrate_choices(playback.max_bitrate_kbps),
            formats: format_choices(playback.transcode_format.as_deref()),
            lyrics: lyrics_sources(&settings.integrations.lyrics_sources),
            settings,
        }
    }

    fn playback(&self, form: PlaybackForm) -> PlaybackSettings {
        let current = &self.settings.playback;
        PlaybackSettings {
            replay_gain: replay_gain_from_index(form.replay_gain),
            gapless: form.gapless,
            audio_device: self
                .devices
                .value(form.device)
                .unwrap_or_else(|| current.audio_device.clone()),
            max_bitrate_kbps: self
                .bitrates
                .value(form.bitrate)
                .unwrap_or(current.max_bitrate_kbps),
            transcode_format: self
                .formats
                .value(form.format)
                .unwrap_or_else(|| current.transcode_format.clone()),
            scrobble: form.scrobble,
            ..current.clone()
        }
    }
}

#[derive(Default)]
struct Shared {
    view: Mutex<View>,
    generation: AtomicU64,
}

impl Shared {
    fn view(&self) -> MutexGuard<'_, View> {
        self.view
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn strings(values: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values
            .iter()
            .map(|v| SharedString::from(v.as_str()))
            .collect::<Vec<_>>(),
    ))
}

fn index(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(0)
}

fn set_lyrics_rows(state: &SettingsState, sources: &[LyricsSource]) {
    let rows: Vec<LyricsSourceRow> = sources
        .iter()
        .map(|s| LyricsSourceRow {
            id: s.id.as_str().into(),
            name: lyrics_source_name(&s.id).into(),
            enabled: s.enabled,
        })
        .collect();
    state.set_lyrics_sources(ModelRc::new(VecModel::from(rows)));
}

fn show(window: &AppWindow, shared: &Shared, snapshot: SettingsSnapshot) {
    let state = window.global::<SettingsState>();
    let view = View::new(snapshot);
    let settings = &view.settings;
    state.set_replay_gain(replay_gain_index(settings.playback.replay_gain));
    state.set_gapless(settings.playback.gapless);
    state.set_scrobble(settings.playback.scrobble);
    state.set_devices(strings(&view.devices.labels));
    state.set_device_index(index(view.devices.selected));
    state.set_bitrates(strings(&view.bitrates.labels));
    state.set_bitrate_index(index(view.bitrates.selected));
    state.set_formats(strings(&view.formats.labels));
    state.set_format_index(index(view.formats.selected));
    state.set_mpris(settings.integrations.mpris);
    state.set_discord(settings.integrations.discord_rich_presence);
    set_lyrics_rows(&state, &view.lyrics);
    state.set_servers(ModelRc::new(VecModel::from(server_rows(settings))));
    state.set_follow_current_song(settings.ui.follow_current_song);
    state.set_switching("".into());
    state.set_loaded(true);
    *shared.view() = view;
}

fn load(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let request = shared.generation.fetch_add(1, Ordering::Relaxed) + 1;
    let (reply, response) = oneshot::channel();
    ctx.send(Command::GetSettings(reply));
    let weak = window.as_weak();
    let shared = Arc::clone(shared);
    ctx.rt.spawn(async move {
        let Ok(snapshot) = response.await else {
            return;
        };
        on_ui(&weak, move |window| {
            if shared.generation.load(Ordering::Relaxed) == request {
                show(window, &shared, snapshot);
            }
        });
    });
}

fn save_integrations(ctx: &Context, view: &mut View) {
    view.settings.integrations.lyrics_sources = enabled_lyrics_sources(&view.lyrics);
    ctx.send(Command::SaveIntegrations(
        view.settings.integrations.clone(),
    ));
}

fn restore(ctx: &Context, config: &ServerConfig) -> Result<Arc<dyn MusicServer>> {
    let credential = ctx.store.credential(&config.id)?.ok_or_else(|| {
        anyhow!(
            "No saved login for {}; remove it and add it again",
            config.name
        )
    })?;
    ctx.connector.restore(config, &credential)
}

fn switch_server(window: &AppWindow, ctx: &Context, shared: &Shared, id: SharedString) {
    let Some(config) = shared
        .view()
        .settings
        .servers
        .iter()
        .find(|s| s.id == id.as_str())
        .cloned()
    else {
        return;
    };
    window.global::<SettingsState>().set_switching(id);
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ctx.rt.clone().spawn(async move {
        let restored = {
            let (ctx, config) = (ctx.clone(), config.clone());
            tokio::task::spawn_blocking(move || restore(&ctx, &config)).await
        };
        let result = restored.map_err(anyhow::Error::from).and_then(|r| r);
        if let Ok(server) = &result {
            ctx.session.set(Some(Arc::clone(server)));
            ctx.send(Command::Connected {
                config,
                others: Vec::new(),
            });
        }
        on_ui(&weak, move |window| {
            let state = window.global::<SettingsState>();
            match result {
                Ok(server) => {
                    login::enter_main(window, &server);
                    state.invoke_load("".into());
                }
                Err(e) => {
                    state.set_switching("".into());
                    show_status(window, &format!("Could not switch server: {e:#}"));
                }
            }
        });
    });
}

fn log_out(window: &AppWindow, ctx: &Context, shared: &Shared) {
    if let Some(id) = shared.view().settings.current_server_id.take() {
        ctx.send(Command::RemoveServer(id));
    }
    ctx.session.set(None);
    window.global::<AppState>().set_server_name("".into());
    login::show_login(window, "");
}

pub fn install(window: &AppWindow, ctx: &Context) {
    install_with(window, ctx, &Arc::new(Shared::default()));
}

fn install_with(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();

    let with = |f: fn(&AppWindow, &Context, &Arc<Shared>)| {
        let weak = window.as_weak();
        let ctx = ctx.clone();
        let shared = Arc::clone(shared);
        move || {
            if let Some(window) = weak.upgrade() {
                f(&window, &ctx, &shared);
            }
        }
    };

    let reload = with(load);
    state.on_load(move |_| reload());

    state.on_playback_changed(with(|window, ctx, shared| {
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
    }));

    state.on_integrations_changed(with(|window, ctx, shared| {
        let state = window.global::<SettingsState>();
        let mut view = shared.view();
        view.settings.integrations.mpris = state.get_mpris();
        view.settings.integrations.discord_rich_presence = state.get_discord();
        save_integrations(ctx, &mut view);
    }));

    let (weak, toggle_ctx, toggle_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_toggle_lyrics_source(move |row| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut view = toggle_shared.view();
        let Some(source) = usize::try_from(row)
            .ok()
            .and_then(|i| view.lyrics.get_mut(i))
        else {
            return;
        };
        source.enabled = !source.enabled;
        set_lyrics_rows(&window.global::<SettingsState>(), &view.lyrics);
        save_integrations(&toggle_ctx, &mut view);
    });

    let (weak, move_ctx, move_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_move_lyrics_source(move |row, delta| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut view = move_shared.view();
        let (Ok(row), Ok(delta)) = (usize::try_from(row), isize::try_from(delta)) else {
            return;
        };
        if move_lyrics_source(&mut view.lyrics, row, delta) {
            set_lyrics_rows(&window.global::<SettingsState>(), &view.lyrics);
            save_integrations(&move_ctx, &mut view);
        }
    });

    state.on_interface_changed(with(|window, ctx, shared| {
        let follow_current_song = window.global::<SettingsState>().get_follow_current_song();
        let queue_panel_open = window.global::<AppState>().get_queue_open();
        let mut view = shared.view();
        view.settings.ui.follow_current_song = follow_current_song;
        view.settings.ui.queue_panel_open = queue_panel_open;
        ctx.send(Command::SaveInterface {
            queue_panel_open,
            follow_current_song,
        });
    }));

    let (weak, switch_ctx, switch_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_switch_server(move |id| {
        if let Some(window) = weak.upgrade() {
            switch_server(&window, &switch_ctx, &switch_shared, id);
        }
    });

    let (weak, remove_ctx, remove_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_remove_server(move |id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let current = remove_shared.view().settings.current_server_id.clone();
        if current.as_deref() == Some(id.as_str()) {
            log_out(&window, &remove_ctx, &remove_shared);
        } else {
            remove_ctx.send(Command::RemoveServer(id.into()));
            load(&window, &remove_ctx, &remove_shared);
        }
    });

    state.on_add_server(with(|window, _, _| login::show_login(window, "")));
    state.on_log_out(with(|window, ctx, shared| log_out(window, ctx, shared)));

    load(window, ctx, shared);
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ServerType;
    use feishin_core::queue::Uid;
    use feishin_player::PlayerEvent;
    use feishin_storage::settings::{IntegrationSettings, UiSettings};
    use slint::Model;
    use tokio::sync::mpsc;

    use super::*;
    use crate::audio::{AudioOutput, Track};
    use crate::connect::DemoConnector;
    use crate::controller::{Controller, Deps, UiSink, UiUpdate};
    use crate::covers::DemoCovers;
    use crate::demo::{FakeServer, demo_credential, test_song};
    use crate::media::NoMediaSession;
    use crate::session::Session;
    use crate::store::{JsonStore, Store};
    use crate::ui::{Page, Route};

    #[derive(Clone, Default)]
    struct RecordingAudio(Arc<Mutex<Vec<String>>>);

    impl RecordingAudio {
        fn log(&self, call: String) -> Result<()> {
            if let Ok(mut calls) = self.0.lock() {
                calls.push(call);
            }
            Ok(())
        }

        fn take(&self) -> Vec<String> {
            self.0
                .lock()
                .map(|mut calls| calls.drain(..).collect())
                .unwrap_or_default()
        }
    }

    impl AudioOutput for RecordingAudio {
        fn load(&self, track: &Track, _start_ms: u64) -> Result<()> {
            self.log(format!("load {}", track.uid))
        }
        fn preload_next(&self, track: &Track) -> Result<()> {
            self.log(format!("preload {}", track.uid))
        }
        fn clear_next(&self) -> Result<()> {
            self.log("clear".into())
        }
        fn play(&self) -> Result<()> {
            Ok(())
        }
        fn pause(&self) -> Result<()> {
            Ok(())
        }
        fn stop(&self) -> Result<()> {
            self.log("stop".into())
        }
        fn seek(&self, _position_ms: u64) -> Result<()> {
            Ok(())
        }
        fn set_volume(&self, _volume: u8) -> Result<()> {
            Ok(())
        }
        fn set_muted(&self, _muted: bool) -> Result<()> {
            Ok(())
        }
        fn set_replay_gain(&self, mode: ReplayGainSetting) -> Result<()> {
            self.log(format!("replay gain {mode:?}"))
        }
        fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
            self.log(format!("device {}", device.unwrap_or("auto")))
        }
        fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
            Ok(vec![device("auto", "Autoselect device")])
        }
    }

    struct NoUi;

    impl UiSink for NoUi {
        fn send(&self, _update: UiUpdate) {}
    }

    struct Harness {
        controller: Controller,
        audio: RecordingAudio,
        store: Arc<JsonStore>,
        session: Session,
        _dir: tempfile::TempDir,
    }

    fn harness(settings: Settings) -> Harness {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e}"));
        let store = Arc::new(JsonStore::new(dir.path()));
        let session = Session::default();
        session.set(Some(Arc::new(FakeServer::new(10))));
        let audio = RecordingAudio::default();
        let deps = Deps {
            audio: Box::new(audio.clone()),
            media: Box::new(NoMediaSession),
            store: store.clone(),
            session: session.clone(),
            ui: Arc::new(NoUi),
            commands: mpsc::unbounded_channel().0,
        };
        Harness {
            controller: Controller::new(deps, settings, None, Some(7)),
            audio,
            store,
            session,
            _dir: dir,
        }
    }

    impl Harness {
        fn start(&mut self) {
            let songs = vec![test_song("a", 100_000), test_song("b", 100_000)];
            self.controller.play_now(songs, "a");
            let uid = self
                .controller
                .queue()
                .current()
                .map_or(Uid::MAX, |i| i.uid);
            self.controller
                .handle(Command::Player(PlayerEvent::Started { uid }));
            self.audio.take();
        }

        fn saved(&self) -> Settings {
            self.store.load_settings().unwrap_or_default()
        }
    }

    fn two_servers() -> Settings {
        Settings {
            servers: vec![FakeServer::config_for_demo(), server("b")],
            current_server_id: Some("demo".into()),
            ..Settings::default()
        }
    }

    #[tokio::test]
    async fn applying_playback_reaches_the_player_and_is_saved() {
        let mut h = harness(Settings::default());
        h.start();
        let next = h.controller.queue().peek_next().map_or(Uid::MAX, |i| i.uid);
        h.controller
            .handle(Command::ApplyPlayback(PlaybackSettings {
                replay_gain: ReplayGainSetting::Album,
                audio_device: Some("pipewire/hdmi".into()),
                max_bitrate_kbps: Some(128),
                volume: 5,
                ..PlaybackSettings::default()
            }));
        assert_eq!(
            h.audio.take(),
            [
                "replay gain Album".to_owned(),
                "device pipewire/hdmi".into(),
                "clear".into(),
                format!("preload {next}"),
            ]
        );
        let saved = h.saved().playback;
        assert_eq!(saved.replay_gain, ReplayGainSetting::Album);
        assert_eq!(saved.max_bitrate_kbps, Some(128));
        assert_eq!(saved.volume, PlaybackSettings::default().volume);

        h.controller
            .handle(Command::ApplyPlayback(PlaybackSettings {
                gapless: false,
                ..saved
            }));
        assert_eq!(h.audio.take(), ["clear"]);
    }

    #[tokio::test]
    async fn removing_servers_forgets_logins_and_disconnects_the_current_one() {
        let mut h = harness(two_servers());
        for id in ["demo", "b"] {
            h.store.set_credential(id, &demo_credential()).ok();
        }
        h.start();

        h.controller.handle(Command::RemoveServer("b".into()));
        assert!(h.store.credential("b").ok().flatten().is_none());
        assert_eq!(h.saved().servers.len(), 1);
        assert!(!h.controller.queue().is_empty());

        h.controller.handle(Command::RemoveServer("demo".into()));
        let saved = h.saved();
        assert!(saved.servers.is_empty() && saved.current_server_id.is_none());
        assert!(h.session.get().is_none());
        assert!(h.controller.queue().is_empty());
        assert_eq!(h.audio.take(), ["stop"]);
    }

    #[tokio::test]
    async fn connecting_to_another_server_clears_the_queue() {
        let mut h = harness(two_servers());
        h.start();
        h.controller.handle(Command::Connected {
            config: FakeServer::config_for_demo(),
            others: Vec::new(),
        });
        assert!(!h.controller.queue().is_empty());
        h.controller.handle(Command::Connected {
            config: server("b"),
            others: Vec::new(),
        });
        assert!(h.controller.queue().is_empty());
        assert_eq!(h.saved().current_server_id.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn interface_and_integrations_are_saved_and_survive_shutdown() {
        let mut h = harness(Settings::default());
        h.controller.handle(Command::SaveInterface {
            queue_panel_open: true,
            follow_current_song: true,
        });
        let integrations = IntegrationSettings {
            discord_rich_presence: true,
            lyrics_sources: vec!["genius".into()],
            ..IntegrationSettings::default()
        };
        h.controller
            .handle(Command::SaveIntegrations(integrations.clone()));
        let (reply, snapshot) = oneshot::channel();
        h.controller.handle(Command::GetSettings(reply));
        let snapshot = snapshot.await.unwrap_or_else(|e| panic!("{e}"));
        assert!(snapshot.settings.ui.follow_current_song);
        assert_eq!(snapshot.devices.len(), 1);

        let (done, _finished) = oneshot::channel();
        h.controller.handle(Command::Shutdown {
            ui: UiSettings::default(),
            done,
        });
        let saved = h.saved();
        assert!(saved.ui.follow_current_song);
        assert!(!saved.ui.queue_panel_open);
        assert_eq!(saved.integrations, integrations);
    }

    #[test]
    fn settings_view_renders_and_saves_changes() {
        i_slint_backend_testing::init_no_event_loop();
        let window = AppWindow::new().unwrap_or_else(|e| panic!("window: {e}"));
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e}"));
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("runtime: {e}"));
        let (commands, mut received) = mpsc::unbounded_channel();
        let ctx = Context {
            rt: rt.handle().clone(),
            commands,
            session: Session::default(),
            store: Arc::new(JsonStore::new(dir.path())),
            connector: Arc::new(DemoConnector),
            covers: Arc::new(DemoCovers),
        };
        let shared = Arc::new(Shared::default());
        install_with(&window, &ctx, &shared);
        let app = window.global::<AppState>();
        app.set_page(Page::Main);
        app.set_route(Route::Settings);
        window.show().unwrap_or_else(|e| panic!("show: {e}"));
        show(
            &window,
            &shared,
            SettingsSnapshot {
                settings: two_servers(),
                devices: vec![device("auto", "Autoselect device"), device("x", "Speakers")],
            },
        );

        for _ in 0..2 {
            assert!(matches!(received.try_recv(), Ok(Command::GetSettings(_))));
        }
        let state = window.global::<SettingsState>();
        let servers: Vec<_> = state.get_servers().iter().collect();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "Demo library");
        assert!(servers[0].current && !servers[1].current);
        let sources: Vec<_> = state.get_lyrics_sources().iter().map(|s| s.name).collect();
        assert_eq!(sources, ["Server", "LRCLIB", "NetEase", "Genius"]);
        let devices: Vec<_> = state.get_devices().iter().collect();
        assert_eq!(devices, ["Autoselect device", "Speakers"]);
        assert!(state.get_loaded());

        state.invoke_toggle_lyrics_source(3);
        match received.try_recv() {
            Ok(Command::SaveIntegrations(saved)) => {
                assert_eq!(
                    saved.lyrics_sources,
                    ["server", "lrclib", "netease", "genius"]
                );
            }
            _ => panic!("expected SaveIntegrations"),
        }

        state.set_replay_gain(1);
        state.set_device_index(1);
        state.invoke_playback_changed();
        match received.try_recv() {
            Ok(Command::ApplyPlayback(playback)) => {
                assert_eq!(playback.replay_gain, ReplayGainSetting::Track);
                assert_eq!(playback.audio_device.as_deref(), Some("x"));
            }
            _ => panic!("expected ApplyPlayback"),
        }

        state.invoke_remove_server("b".into());
        assert!(matches!(received.try_recv(), Ok(Command::RemoveServer(id)) if id == "b"));
        assert!(matches!(received.try_recv(), Ok(Command::GetSettings(_))));
        state.invoke_log_out();
        assert!(matches!(received.try_recv(), Ok(Command::RemoveServer(id)) if id == "demo"));
        assert_eq!(app.get_page(), Page::Login);
    }

    fn device(name: &str, description: &str) -> AudioDevice {
        AudioDevice {
            name: name.into(),
            description: description.into(),
        }
    }

    fn server(id: &str) -> ServerConfig {
        ServerConfig {
            id: id.into(),
            name: format!("Server {id}"),
            server_type: ServerType::Navidrome,
            url: format!("https://{id}.example.com"),
            username: "me".into(),
        }
    }

    #[test]
    fn bitrates_offer_original_and_keep_custom_values() {
        let choices = bitrate_choices(None);
        assert_eq!(choices.labels[0], "Original");
        assert_eq!(choices.selected, 0);
        assert_eq!(choices.value(1), Some(Some(320)));
        let custom = bitrate_choices(Some(500));
        assert_eq!(custom.labels[custom.selected], "500 kbps");
        assert_eq!(bitrate_choices(Some(128)).labels[5], "128 kbps");
        assert_eq!(custom.value(-1), None);
    }

    #[test]
    fn formats_are_normalized_and_custom_ones_kept() {
        let choices = format_choices(Some(" OPUS "));
        assert_eq!(choices.labels[choices.selected], "opus");
        let custom = format_choices(Some("alac"));
        assert_eq!(
            custom.value(index(custom.selected)),
            Some(Some("alac".into()))
        );
        assert_eq!(format_choices(Some("")).selected, 0);
    }

    #[test]
    fn devices_map_auto_to_the_default_and_keep_missing_ones() {
        let devices = [
            device("auto", "Autoselect device"),
            device("pipewire/hdmi", "HDMI"),
            device("alsa/raw", ""),
        ];
        let choices = device_choices(&devices, None);
        assert_eq!(choices.labels, ["Autoselect device", "HDMI", "alsa/raw"]);
        assert_eq!(choices.selected, 0);
        assert_eq!(choices.value(0), Some(None));

        let selected = device_choices(&devices, Some("pipewire/hdmi"));
        assert_eq!(selected.selected, 1);

        let missing = device_choices(&devices[1..], Some("pulse/gone"));
        assert_eq!(missing.labels[0], "System default");
        assert_eq!(missing.labels[missing.selected], "pulse/gone (unavailable)");
    }

    #[test]
    fn replay_gain_round_trips() {
        for setting in [
            ReplayGainSetting::Off,
            ReplayGainSetting::Track,
            ReplayGainSetting::Album,
        ] {
            assert_eq!(replay_gain_from_index(replay_gain_index(setting)), setting);
        }
        assert_eq!(replay_gain_from_index(7), ReplayGainSetting::Off);
    }

    #[test]
    fn lyrics_sources_list_enabled_first_then_the_rest() {
        let mut sources = lyrics_sources(&["lrclib".into(), "server".into(), "lrclib".into()]);
        let ids: Vec<_> = sources.iter().map(|s| (s.id.as_str(), s.enabled)).collect();
        assert_eq!(
            ids,
            [
                ("lrclib", true),
                ("server", true),
                ("netease", false),
                ("genius", false)
            ]
        );
        assert!(move_lyrics_source(&mut sources, 1, -1));
        assert!(!move_lyrics_source(&mut sources, 0, -1));
        assert!(!move_lyrics_source(&mut sources, 3, 1));
        sources[3].enabled = true;
        assert_eq!(
            enabled_lyrics_sources(&sources),
            ["server", "lrclib", "genius"]
        );
        assert_eq!(lyrics_source_name("netease"), "NetEase");
        assert_eq!(lyrics_source_name("custom"), "custom");
    }

    #[test]
    fn default_lyrics_sources_survive_a_round_trip() {
        let defaults = IntegrationSettings::default().lyrics_sources;
        assert_eq!(enabled_lyrics_sources(&lyrics_sources(&defaults)), defaults);
    }

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

    #[test]
    fn playback_form_maps_to_settings_and_keeps_volume() {
        let mut settings = Settings::default();
        settings.playback.volume = 12;
        settings.playback.audio_device = Some("pipewire/hdmi".into());
        let view = View::new(SettingsSnapshot {
            settings,
            devices: vec![
                device("auto", "Autoselect"),
                device("pipewire/hdmi", "HDMI"),
            ],
        });
        let form = PlaybackForm {
            replay_gain: 2,
            gapless: false,
            device: 0,
            bitrate: 3,
            format: 2,
            scrobble: false,
        };
        let playback = view.playback(form);
        assert_eq!(playback.replay_gain, ReplayGainSetting::Album);
        assert!(!playback.gapless && !playback.scrobble);
        assert_eq!(playback.audio_device, None);
        assert_eq!(playback.max_bitrate_kbps, Some(192));
        assert_eq!(playback.transcode_format.as_deref(), Some("opus"));
        assert_eq!(playback.volume, 12);

        let out_of_range = view.playback(PlaybackForm { device: 99, ..form });
        assert_eq!(out_of_range.audio_device.as_deref(), Some("pipewire/hdmi"));
    }
}
