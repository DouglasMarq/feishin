use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use feishin_api::{Credential, MusicServer};
use feishin_core::model::ServerConfig;
use feishin_storage::queue_store::StoredQueue;
use feishin_storage::settings::Settings;
use slint::ComponentHandle;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};

use crate::audio::{AudioOutput, FakeAudio, MpvOutput};
use crate::connect::{Connector, DemoConnector, ServerConnector};
use crate::controller::{Command, Controller, Deps, forward_command};
use crate::covers::{CachedCovers, CoverProvider, DemoCovers};
use crate::demo::{DemoLibrary, FakeServer, demo_credential};
use crate::discord::DiscordSession;
use crate::media::{MediaSession, MediaSessions, Mpris, NoMediaSession};
use crate::session::Session;
use crate::sleep_timer::{self, SleepTimerUi};
use crate::store::{JsonStore, Store, SystemStore};
use crate::ui::{self, AppWindow, Context, login};

const SHUTDOWN_SAVE_TIMEOUT: Duration = Duration::from_secs(3);
const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Real,
    Demo(DemoLibrary),
}

struct Backends {
    store: Arc<dyn Store>,
    connector: Arc<dyn Connector>,
    covers: Arc<dyn CoverProvider>,
}

type SavedLogin = (ServerConfig, Result<Option<Credential>>);

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
    }
}

fn real_backends() -> Result<Backends> {
    let store = SystemStore::from_env()?;
    let covers = CachedCovers::new(store.paths())?;
    Ok(Backends {
        store: Arc::new(store),
        connector: Arc::new(ServerConnector),
        covers: Arc::new(covers),
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
        Mode::Demo(_) => Box::new(FakeAudio::new(on_event)),
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

fn enter_restored_session(
    window: &AppWindow,
    context: &Context,
    config: ServerConfig,
    server: Arc<dyn MusicServer>,
) {
    context.session.set(Some(Arc::clone(&server)));
    context.send(Command::Connected {
        config,
        others: Vec::new(),
    });
    login::enter_main(window, &server);
}

fn restore_session(window: &AppWindow, context: &Context, settings: &Settings, mode: Mode) {
    let restored = match saved_login(context, settings, mode) {
        Some((config, Ok(Some(credential)))) => context
            .connector
            .restore(&config, &credential)
            .map(|server| (config, server)),
        Some((_, Ok(None))) | None => {
            login::show_login(window, "");
            return;
        }
        Some((_, Err(error))) => Err(error),
    };
    match restored {
        Ok((config, server)) => enter_restored_session(window, context, config, server),
        Err(error) => login::show_login(window, &format!("{error:#}")),
    }
}

fn build_runtime() -> Result<Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
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

fn load_stored_queue(store: &dyn Store) -> Option<StoredQueue> {
    store.load_queue().unwrap_or_else(|error| {
        tracing::warn!("loading the saved queue failed: {error:#}");
        None
    })
}

fn app_context(
    runtime: &Runtime,
    backends: Backends,
    commands: &mpsc::UnboundedSender<Command>,
) -> Context {
    Context {
        rt: runtime.handle().clone(),
        commands: commands.clone(),
        session: Session::default(),
        store: backends.store,
        connector: backends.connector,
        covers: backends.covers,
    }
}

fn controller_deps(
    mode: Mode,
    settings: &Settings,
    window: &AppWindow,
    context: &Context,
) -> Result<Deps> {
    Ok(Deps {
        audio: audio_output(mode, settings, &context.commands)?,
        media: media_session(mode, settings, &context.commands),
        store: Arc::clone(&context.store),
        session: context.session.clone(),
        ui: Arc::new(SleepTimerUi::new(
            ui::player::SlintSink::new(window.as_weak(), context.clone()),
            window.as_weak(),
        )),
        commands: context.commands.clone(),
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
    let stored_queue = load_stored_queue(backends.store.as_ref());

    let window = AppWindow::new().context("creating the window")?;
    let (commands, receiver) = mpsc::unbounded_channel();
    let context = app_context(&runtime, backends, &commands);
    ui::install(&window, &context, &settings);
    sleep_timer::install(&window, &context);

    let deps = controller_deps(mode, &settings, &window, &context)?;
    let controller = Controller::new(deps, settings.clone(), stored_queue, None);
    runtime.spawn(controller.run(receiver));

    restore_session(&window, &context, &settings, mode);
    window.run().context("running the window")?;

    save_state_on_exit(&runtime, &window, &context, &settings);
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_TIMEOUT);
    Ok(())
}
