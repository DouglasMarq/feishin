use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use feishin_api::{Credential, MusicServer};
use feishin_core::model::ServerConfig;
use feishin_storage::settings::Settings;
use slint::ComponentHandle;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot, watch};

use crate::audio::{AudioOutput, MpvOutput};
use crate::connect::{Connector, DemoConnector, ServerConnector};
use crate::controller::{Command, Controller, Deps, forward_command, load_saved_queue};
use crate::covers::{CachedCovers, CoverProvider, DemoCovers};
use crate::demo::{DemoLibrary, FakeAudio, FakeServer, demo_credential};
use crate::discord::DiscordSession;
use crate::discover::{DemoDiscover, DiscoverSource, YouTubeMusicSource};
use crate::media::{MediaSession, MediaSessions, Mpris, NoMediaSession, SleepInhibition};
use crate::remote::{self, HostFeed, LocalGate, Remote, RemoteStart};
use crate::session::Session;
use crate::sleep_timer::{self, SleepTimerUi};
use crate::store::{JsonStore, Store, SystemStore};
use crate::termination;
use crate::ui::{self, AppWindow, Context, login, on_ui};
use feishin_remote as remote_state;

const SHUTDOWN_SAVE_TIMEOUT: Duration = Duration::from_secs(6);
const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const IO_WORKER_THREADS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Real,
    Demo(DemoLibrary),
}

struct Backends {
    store: Arc<dyn Store>,
    connector: Arc<dyn Connector>,
    covers: Arc<dyn CoverProvider>,
    discover: Arc<dyn DiscoverSource>,
}

type SavedLogin = (ServerConfig, Result<Option<Credential>>);
type RestoredLogin = Option<(ServerConfig, Arc<dyn MusicServer>)>;

fn backends(mode: Mode) -> Result<Backends> {
    match mode {
        Mode::Demo(library) => Ok(demo_backends(library)),
        Mode::Real => real_backends(),
    }
}

fn demo_backends(library: DemoLibrary) -> Backends {
    Backends {
        store: Arc::new(JsonStore::new(std::env::temp_dir().join("feishin-rs-demo"))),
        connector: Arc::new(DemoConnector::new(library)),
        covers: Arc::new(DemoCovers),
        discover: Arc::new(DemoDiscover::new(library)),
    }
}

fn real_backends() -> Result<Backends> {
    let store = SystemStore::from_env()?;
    let covers = CachedCovers::new(store.paths())?;
    Ok(Backends {
        store: Arc::new(store),
        connector: Arc::new(ServerConnector::new()?),
        covers: Arc::new(covers),
        discover: Arc::new(YouTubeMusicSource::default()),
    })
}

fn audio_output(
    mode: Mode,
    settings: &Settings,
    commands: &mpsc::UnboundedSender<Command>,
) -> Result<Box<dyn AudioOutput>> {
    let commands = commands.clone();
    let on_event = Box::new(move |event| {
        forward_command(&commands, Command::Player(event));
    });
    Ok(match mode {
        Mode::Demo(_) => {
            Box::new(FakeAudio::new(on_event).with_crossfade(settings.playback.crossfade()))
        }
        Mode::Real => Box::new(MpvOutput::new(&settings.playback, on_event)?),
    })
}

fn mpris_session(commands: &mpsc::UnboundedSender<Command>) -> Option<Box<dyn MediaSession>> {
    let commands = commands.clone();
    let on_event = Box::new(move |event| {
        forward_command(&commands, Command::Media(event));
    });
    Mpris::new(on_event)
        .inspect_err(|error| tracing::warn!("MPRIS unavailable: {error:#}"))
        .ok()
        .map(|mpris| Box::new(mpris) as Box<dyn MediaSession>)
}

fn sleep_inhibition() -> Option<Box<dyn MediaSession>> {
    SleepInhibition::new()
        .inspect_err(|error| tracing::warn!("sleep inhibition unavailable: {error:#}"))
        .ok()
        .map(|inhibition| Box::new(inhibition) as Box<dyn MediaSession>)
}

fn discord_session(settings: &Settings) -> Option<Box<dyn MediaSession>> {
    let client_id = settings
        .integrations
        .discord_client_id
        .clone()
        .unwrap_or_default();
    DiscordSession::new(client_id)
        .inspect_err(|error| tracing::warn!("Discord presence unavailable: {error:#}"))
        .ok()
        .map(|discord| Box::new(discord) as Box<dyn MediaSession>)
}

fn combine_media_sessions(mut sessions: Vec<Box<dyn MediaSession>>) -> Box<dyn MediaSession> {
    match sessions.len() {
        0 | 1 => sessions.pop().unwrap_or_else(|| Box::new(NoMediaSession)),
        _ => Box::new(MediaSessions(sessions)),
    }
}

fn media_session(
    mode: Mode,
    settings: &Settings,
    commands: &mpsc::UnboundedSender<Command>,
) -> Box<dyn MediaSession> {
    let mut sessions: Vec<Box<dyn MediaSession>> = Vec::new();
    if mode == Mode::Real && settings.integrations.mpris {
        sessions.extend(mpris_session(commands));
    }
    if mode == Mode::Real {
        sessions.extend(sleep_inhibition());
    }
    if settings.integrations.discord_rich_presence {
        sessions.extend(discord_session(settings));
    }
    combine_media_sessions(sessions)
}

fn demo_login(context: &Context) -> SavedLogin {
    let config = FakeServer::config_for_demo();
    if let Err(error) = context.store.set_credential(&config.id, &demo_credential()) {
        tracing::warn!("saving the demo credential failed: {error:#}");
    }
    (config, Ok(Some(demo_credential())))
}

fn last_used_login(context: &Context, settings: &Settings) -> Option<SavedLogin> {
    settings
        .current_server_id
        .as_deref()
        .and_then(|server_id| {
            settings
                .servers
                .iter()
                .find(|server| server.id == server_id)
        })
        .map(|config| (config.clone(), context.store.credential(&config.id)))
}

fn saved_login(context: &Context, settings: &Settings, mode: Mode) -> Option<SavedLogin> {
    match mode {
        Mode::Demo(_) => Some(demo_login(context)),
        Mode::Real => last_used_login(context, settings),
    }
}

fn restored_login(context: &Context, settings: &Settings, mode: Mode) -> Result<RestoredLogin> {
    let Some((config, credential)) = saved_login(context, settings, mode) else {
        return Ok(None);
    };
    let Some(credential) = credential? else {
        return Ok(None);
    };
    let server = context.connector.restore(&config, &credential)?;
    Ok(Some((config, server)))
}

fn show_restored_session(window: &AppWindow, restored: Result<RestoredLogin>) {
    match restored {
        Ok(Some((_, server))) => login::enter_main(window, &server),
        Ok(None) => login::show_login(window, ""),
        Err(error) => login::show_login(window, &format!("{error:#}")),
    }
}

fn restore_session(window: &AppWindow, context: &Context, settings: &Settings, mode: Mode) {
    let (weak_window, context, settings) = (window.as_weak(), context.clone(), settings.clone());
    let runtime = context.rt.clone();
    runtime.spawn(async move {
        let reading = context.clone();
        let restored =
            tokio::task::spawn_blocking(move || restored_login(&reading, &settings, mode))
                .await
                .map_err(anyhow::Error::from)
                .and_then(|restored| restored);
        if let Ok(Some((config, server))) = &restored {
            login::enter_session(&context, server, config.clone(), None, Vec::new());
        }
        on_ui(&weak_window, move |window| {
            show_restored_session(window, restored);
        });
    });
}

fn build_runtime() -> Result<Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(IO_WORKER_THREADS)
        .thread_name("feishin-io")
        .enable_all()
        .build()?)
}

fn load_settings_or_default(store: &dyn Store) -> Settings {
    store.load_settings().unwrap_or_else(|error| {
        tracing::warn!("loading settings failed, using defaults: {error:#}");
        Settings::default()
    })
}

fn restore_saved_queue_once_shown(context: &Context) {
    let runtime = context.rt.clone();
    let store = Arc::clone(&context.store);
    let commands = context.commands.clone();
    slint::Timer::single_shot(Duration::ZERO, move || {
        runtime.spawn(load_saved_queue(store, commands));
    });
}

fn app_context(
    runtime: &Runtime,
    backends: Backends,
    commands: &mpsc::UnboundedSender<Command>,
    remote: Remote,
) -> Context {
    Context {
        rt: runtime.handle().clone(),
        commands: commands.clone(),
        session: Session::default(),
        store: backends.store,
        connector: backends.connector,
        covers: backends.covers,
        remote,
    }
}

fn controller_deps(
    mode: Mode,
    settings: &Settings,
    window: &AppWindow,
    context: &Context,
    host_state: watch::Sender<remote_state::HostState>,
) -> Result<Deps> {
    let shown = LocalGate::new(
        ui::player::SlintSink::new(window.as_weak(), context.clone()),
        context.remote.target.clone(),
    );
    Ok(Deps {
        audio: audio_output(mode, settings, &context.commands)?,
        media: media_session(mode, settings, &context.commands),
        store: Arc::clone(&context.store),
        session: context.session.clone(),
        ui: Arc::new(SleepTimerUi::new(
            HostFeed::new(shown, host_state),
            window.as_weak(),
        )),
        commands: context.commands.clone(),
        covers: Arc::clone(&context.covers),
    })
}

fn save_state_on_exit(
    runtime: &Runtime,
    window: &AppWindow,
    context: &Context,
    settings: &Settings,
) {
    let (done, finished) = oneshot::channel();
    context.send(Command::Shutdown {
        ui: ui::ui_settings(window, &settings.ui),
        done,
    });
    runtime.block_on(async {
        if tokio::time::timeout(SHUTDOWN_SAVE_TIMEOUT, finished)
            .await
            .is_err()
        {
            tracing::warn!("timed out saving state on exit");
        }
    });
}

pub fn run(mode: Mode) -> Result<()> {
    let runtime = build_runtime()?;
    let backends = backends(mode)?;
    let settings = load_settings_or_default(backends.store.as_ref());

    let window = AppWindow::new().context("creating the window")?;
    crate::i18n::select_language(settings.ui.language.as_deref());
    crate::identity::announce_xdg_app_id();
    let (commands, receiver) = mpsc::unbounded_channel();
    let (remote_requests, remote_receiver) = mpsc::unbounded_channel();
    let (host_state, host_state_receiver) = watch::channel(remote_state::HostState::default());
    let discover = Arc::clone(&backends.discover);
    let context = app_context(&runtime, backends, &commands, Remote::new(remote_requests));
    ui::install(&window, &context, &settings);
    ui::appearance::watch_system_scheme(&window, &context);
    ui::discover::install(&window, &context, discover);
    sleep_timer::install(&window, &context);

    let deps = controller_deps(mode, &settings, &window, &context, host_state)?;
    let mut controller = Controller::new(deps, settings.clone(), None, None);
    controller.expect_saved_queue();
    runtime.spawn(controller.run(receiver));
    remote::start(
        &context,
        RemoteStart {
            mode,
            hosting: settings.integrations.remote_control,
            window: window.as_weak(),
            host_state: host_state_receiver,
            mirror: Arc::new(ui::player::SlintSink::new(
                window.as_weak(),
                context.clone(),
            )),
            requests: remote_receiver,
        },
    );

    restore_session(&window, &context, &settings, mode);
    restore_saved_queue_once_shown(&context);
    termination::quit_on_termination_signals(runtime.handle());
    window.run().context("running the window")?;

    save_state_on_exit(&runtime, &window, &context, &settings);
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_TIMEOUT);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_with(store: Arc<JsonStore>) -> Context {
        let (commands, _ignored) = mpsc::unbounded_channel();
        Context {
            rt: tokio::runtime::Handle::current(),
            commands,
            session: Session::default(),
            store,
            connector: Arc::new(DemoConnector::default()),
            covers: Arc::new(DemoCovers),
            remote: Remote::default(),
        }
    }

    fn last_used_demo_server() -> Settings {
        let config = FakeServer::config_for_demo();
        Settings {
            current_server_id: Some(config.id.clone()),
            servers: vec![config],
            ..Settings::default()
        }
    }

    #[tokio::test]
    async fn given_a_saved_login_of_the_last_server_the_session_is_restored() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonStore::new(directory.path()));
        store.set_credential("demo", &demo_credential()).unwrap();
        let context = context_with(store);

        let restored = restored_login(&context, &last_used_demo_server(), Mode::Real).unwrap();

        let (config, server) = restored.unwrap();
        assert_eq!(config, FakeServer::config_for_demo());
        assert_eq!(server.config().id, "demo");
    }

    #[tokio::test]
    async fn given_no_saved_login_nothing_is_restored() {
        let directory = tempfile::tempdir().unwrap();
        let context = context_with(Arc::new(JsonStore::new(directory.path())));

        let restored = restored_login(&context, &last_used_demo_server(), Mode::Real).unwrap();

        assert!(restored.is_none());
        let without_server = restored_login(&context, &Settings::default(), Mode::Real).unwrap();
        assert!(without_server.is_none());
    }
}
