use std::sync::Arc;

use anyhow::{Result, anyhow};
use feishin_api::MusicServer;
use feishin_core::model::ServerConfig;
use slint::{ComponentHandle, SharedString};

use super::loading::load;
use super::view::Shared;
use crate::controller::Command;
use crate::ui::{AppState, AppWindow, Context, SettingsState, login, on_ui, show_status};

fn restore(ctx: &Context, config: &ServerConfig) -> Result<Arc<dyn MusicServer>> {
    let credential = ctx.store.credential(&config.id)?.ok_or_else(|| {
        anyhow!(
            "No saved login for {}; remove it and add it again",
            config.name
        )
    })?;
    ctx.connector.restore(config, &credential)
}

fn configured_server(shared: &Shared, id: &str) -> Option<ServerConfig> {
    shared
        .view()
        .settings
        .servers
        .iter()
        .find(|server| server.id == id)
        .cloned()
}

pub(super) fn switch_server(window: &AppWindow, ctx: &Context, shared: &Shared, id: SharedString) {
    let Some(config) = configured_server(shared, id.as_str()) else {
        return;
    };
    window.global::<SettingsState>().set_switching(id);
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    ctx.rt.clone().spawn(async move {
        let result = restore_in_background(&ctx, &config).await;
        if let Ok(server) = &result {
            ctx.session.set(Some(Arc::clone(server)));
            ctx.send(Command::Connected {
                config,
                others: Vec::new(),
            });
        }
        on_ui(&weak_window, move |window| {
            show_switch_result(window, result);
        });
    });
}

async fn restore_in_background(
    ctx: &Context,
    config: &ServerConfig,
) -> Result<Arc<dyn MusicServer>> {
    let (ctx, config) = (ctx.clone(), config.clone());
    let restored = tokio::task::spawn_blocking(move || restore(&ctx, &config)).await;
    restored
        .map_err(anyhow::Error::from)
        .and_then(|restore_result| restore_result)
}

fn show_switch_result(window: &AppWindow, result: Result<Arc<dyn MusicServer>>) {
    let state = window.global::<SettingsState>();
    match result {
        Ok(server) => {
            login::enter_main(window, &server);
            state.invoke_load("".into());
        }
        Err(error) => {
            state.set_switching("".into());
            show_status(window, &format!("Could not switch server: {error:#}"));
        }
    }
}

pub(super) fn log_out(window: &AppWindow, ctx: &Context, shared: &Shared) {
    if let Some(id) = shared.view().settings.current_server_id.take() {
        ctx.send(Command::RemoveServer(id));
    }
    ctx.session.set(None);
    window.global::<AppState>().set_server_name("".into());
    login::show_login(window, "");
}

pub(super) fn remove_server(
    window: &AppWindow,
    ctx: &Context,
    shared: &Arc<Shared>,
    id: &SharedString,
) {
    let current = shared.view().settings.current_server_id.clone();
    if current.as_deref() == Some(id.as_str()) {
        log_out(window, ctx, shared);
        return;
    }
    ctx.send(Command::RemoveServer(id.into()));
    load(window, ctx, shared);
}
