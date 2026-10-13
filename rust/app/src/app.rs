//! Startup and shutdown: builds the backends for real or demo mode, wires them and runs the window.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use feishin_storage::settings::Settings;
use slint::ComponentHandle;
use tokio::sync::{mpsc, oneshot};

use crate::audio::{AudioOutput, FakeAudio, MpvOutput};
use crate::connect::{Connector, DemoConnector, ServerConnector};
use crate::controller::{Command, Controller, Deps};
use crate::covers::{CachedCovers, CoverProvider, DemoCovers};
use crate::demo::{FakeServer, demo_credential};
use crate::discord::DiscordSession;
use crate::media::{MediaSession, MediaSessions, Mpris, NoMediaSession};
use crate::session::Session;
use crate::sleep_timer::{self, SleepTimerUi};
use crate::store::{JsonStore, Store, SystemStore};
use crate::ui::{self, AppWindow, Context, login};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Real,
    Demo,
}

struct Backends {
    store: Arc<dyn Store>,
    connector: Arc<dyn Connector>,
    covers: Arc<dyn CoverProvider>,
}

fn backends(mode: Mode) -> Result<Backends> {
    Ok(match mode {
        Mode::Demo => Backends {
            store: Arc::new(JsonStore::new(std::env::temp_dir().join("feishin-rs-demo"))),
            connector: Arc::new(DemoConnector),
            covers: Arc::new(DemoCovers),
        },
        Mode::Real => {
            let store = SystemStore::from_env()?;
            let covers = CachedCovers::new(store.paths())?;
            Backends {
                store: Arc::new(store),
                connector: Arc::new(ServerConnector),
                covers: Arc::new(covers),
            }
        }
    })
}

fn audio_output(
    mode: Mode,
    settings: &Settings,
    commands: &mpsc::UnboundedSender<Command>,
) -> Result<Box<dyn AudioOutput>> {
    let commands = commands.clone();
    let on_event = Box::new(move |event| {
        let _ = commands.send(Command::Player(event));
    });
    Ok(match mode {
        Mode::Demo => Box::new(FakeAudio::new(on_event)),
        Mode::Real => Box::new(MpvOutput::new(&settings.playback, on_event)?),
    })
}

fn media_session(
    mode: Mode,
    settings: &Settings,
    commands: &mpsc::UnboundedSender<Command>,
) -> Box<dyn MediaSession> {
    let mut sessions: Vec<Box<dyn MediaSession>> = Vec::new();
    if mode == Mode::Real && settings.integrations.mpris {
        let commands = commands.clone();
        let on_event = Box::new(move |event| {
            let _ = commands.send(Command::Media(event));
        });
        match Mpris::new(on_event) {
            Ok(mpris) => sessions.push(Box::new(mpris)),
            Err(e) => tracing::warn!("MPRIS unavailable: {e:#}"),
        }
    }
    if settings.integrations.discord_rich_presence {
        let client_id = settings
            .integrations
            .discord_client_id
            .clone()
            .unwrap_or_default();
        match DiscordSession::new(client_id) {
            Ok(discord) => sessions.push(Box::new(discord)),
            Err(e) => tracing::warn!("Discord presence unavailable: {e:#}"),
        }
    }
    match sessions.len() {
        0 => Box::new(NoMediaSession),
        1 => sessions.remove(0),
        _ => Box::new(MediaSessions(sessions)),
    }
}

/// Reconnects to the last used server, or shows the login screen.
fn restore_session(window: &AppWindow, ctx: &Context, settings: &Settings, mode: Mode) {
    let saved = match mode {
        Mode::Demo => Some((FakeServer::config_for_demo(), Ok(Some(demo_credential())))),
        Mode::Real => settings
            .current_server_id
            .as_deref()
            .and_then(|id| settings.servers.iter().find(|s| s.id == id))
            .map(|config| (config.clone(), ctx.store.credential(&config.id))),
    };
    let restored = match saved {
        Some((config, Ok(Some(credential)))) => ctx
            .connector
            .restore(&config, &credential)
            .map(|server| (config, server)),
        Some((_, Ok(None))) | None => {
            login::show_login(window, "");
            return;
        }
        Some((_, Err(e))) => Err(e),
    };
    match restored {
        Ok((config, server)) => {
            ctx.session.set(Some(Arc::clone(&server)));
            ctx.send(Command::Connected {
                config,
                others: Vec::new(),
            });
            login::enter_main(window, &server);
        }
        Err(e) => login::show_login(window, &format!("{e:#}")),
    }
}

pub fn run(mode: Mode) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("feishin-io")
        .enable_all()
        .build()?;
    let Backends {
        store,
        connector,
        covers,
    } = backends(mode)?;
    let settings = store.load_settings().unwrap_or_else(|e| {
        tracing::warn!("loading settings failed, using defaults: {e:#}");
        Settings::default()
    });
    let stored_queue = store.load_queue().unwrap_or_else(|e| {
        tracing::warn!("loading the saved queue failed: {e:#}");
        None
    });

    let window = AppWindow::new().context("creating the window")?;
    let (commands, receiver) = mpsc::unbounded_channel();
    let ctx = Context {
        rt: rt.handle().clone(),
        commands: commands.clone(),
        session: Session::default(),
        store: Arc::clone(&store),
        connector,
        covers,
    };
    ui::install(&window, &ctx, &settings);
    sleep_timer::install(&window, &ctx);

    let deps = Deps {
        audio: audio_output(mode, &settings, &commands)?,
        media: media_session(mode, &settings, &commands),
        store,
        session: ctx.session.clone(),
        ui: Arc::new(SleepTimerUi::new(
            ui::player::SlintSink::new(window.as_weak(), ctx.clone()),
            window.as_weak(),
        )),
        commands: commands.clone(),
    };
    let controller = Controller::new(deps, settings.clone(), stored_queue, None);
    rt.spawn(controller.run(receiver));

    restore_session(&window, &ctx, &settings, mode);
    window.run().context("running the window")?;

    let (done, finished) = oneshot::channel();
    ctx.send(Command::Shutdown {
        ui: ui::ui_settings(&window, &settings.ui),
        done,
    });
    rt.block_on(async {
        if tokio::time::timeout(Duration::from_secs(3), finished)
            .await
            .is_err()
        {
            tracing::warn!("timed out saving state on exit");
        }
    });
    rt.shutdown_timeout(Duration::from_secs(1));
    Ok(())
}
