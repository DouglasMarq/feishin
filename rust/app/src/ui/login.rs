use std::sync::Arc;

use anyhow::{Result, anyhow};
use feishin_api::MusicServer;
use feishin_core::model::{ServerConfig, ServerType};
use feishin_storage::feishin_import::ImportedServer;
use slint::ComponentHandle;

use super::{AppState, AppWindow, Context, LoginState, Page, on_ui, sharing, tracks};
use crate::controller::Command;

fn server_type(index: i32) -> ServerType {
    match index {
        1 => ServerType::Subsonic,
        2 => ServerType::Jellyfin,
        _ => ServerType::Navidrome,
    }
}

fn new_server_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

pub fn normalize_url(input: &str) -> Option<String> {
    let url = input.trim().trim_end_matches('/');
    match (url.is_empty(), url.contains("://")) {
        (true, _) => None,
        (false, true) => Some(url.to_owned()),
        (false, false) => Some(format!("https://{url}")),
    }
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest).to_owned()
}

pub fn enter_main(window: &AppWindow, server: &Arc<dyn MusicServer>) {
    let app = window.global::<AppState>();
    app.set_server_name(server.config().name.as_str().into());
    sharing::show_availability(window, server.as_ref());
    app.set_page(Page::Main);
    tracks::refresh(window);
}

pub fn show_login(window: &AppWindow, error: &str) {
    window.global::<LoginState>().set_error(error.into());
    window.global::<AppState>().set_page(Page::Login);
}

fn finish(window: &slint::Weak<AppWindow>, result: Result<Arc<dyn MusicServer>>) {
    on_ui(window, move |window| {
        let login = window.global::<LoginState>();
        login.set_busy(false);
        match result {
            Ok(server) => {
                login.set_password("".into());
                login.set_error("".into());
                enter_main(window, &server);
            }
            Err(error) => login.set_error(format!("{error:#}").into()),
        }
    });
}

async fn login(
    ctx: &Context,
    config: ServerConfig,
    password: String,
) -> Result<Arc<dyn MusicServer>> {
    let (server, credential) = ctx.connector.login(config.clone(), &password).await?;
    ctx.session.set(Some(Arc::clone(&server)));
    ctx.send(Command::Connected {
        config,
        credential: Some(credential),
        others: Vec::new(),
    });
    Ok(server)
}

fn first_restored_session(
    ctx: &Context,
    imported: &[ImportedServer],
) -> Option<(ServerConfig, Arc<dyn MusicServer>)> {
    imported.iter().find_map(|server| {
        let credential = server.credential.as_ref()?;
        ctx.connector
            .restore(&server.config, credential)
            .inspect_err(|error| tracing::warn!("{}: {error:#}", server.config.name))
            .ok()
            .map(|session| (server.config.clone(), session))
    })
}

async fn import(ctx: &Context) -> Result<Arc<dyn MusicServer>> {
    let store = Arc::clone(&ctx.store);
    let imported = tokio::task::spawn_blocking(move || store.import_feishin()).await??;
    let (config, server) = first_restored_session(ctx, &imported).ok_or_else(|| {
        anyhow!(tr_plural!(
            "Found {n} server in Feishin but it has no usable saved login",
            "Found {n} servers in Feishin but none with a usable saved login",
            imported.len()
        ))
    })?;
    ctx.session.set(Some(Arc::clone(&server)));
    ctx.send(Command::Connected {
        config,
        credential: None,
        others: imported,
    });
    Ok(server)
}

fn config_from_form(state: &LoginState<'_>, url: String) -> ServerConfig {
    ServerConfig {
        id: new_server_id(),
        name: host_of(&url),
        server_type: server_type(state.get_server_type()),
        url,
        username: state.get_username().trim().to_owned(),
    }
}

fn install_login(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let login_ctx = ctx.clone();
    window.global::<LoginState>().on_login(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<LoginState>();
        let Some(url) = normalize_url(&state.get_url()) else {
            state.set_error(tr!("Enter the server URL").into());
            return;
        };
        let config = config_from_form(&state, url);
        let password = state.get_password().to_string();
        state.set_busy(true);
        state.set_error("".into());
        let ctx = login_ctx.clone();
        let weak = weak.clone();
        login_ctx.rt.spawn(async move {
            let result = login(&ctx, config, password).await;
            finish(&weak, result);
        });
    });
}

fn install_import(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let import_ctx = ctx.clone();
    window.global::<LoginState>().on_import_feishin(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<LoginState>();
        state.set_busy(true);
        state.set_error("".into());
        let ctx = import_ctx.clone();
        let weak = weak.clone();
        import_ctx.rt.spawn(async move {
            let result = import(&ctx).await;
            finish(&weak, result);
        });
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    install_login(window, ctx);
    install_import(window, ctx);
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::connect::DemoConnector;
    use crate::covers::DemoCovers;
    use crate::demo::{FakeServer, demo_credential};
    use crate::session::Session;
    use crate::store::{JsonStore, Store};

    #[tokio::test]
    async fn given_a_login_the_credential_travels_to_the_controller() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonStore::new(directory.path()));
        let (commands, mut received) = mpsc::unbounded_channel();
        let ctx = Context {
            rt: tokio::runtime::Handle::current(),
            commands,
            session: Session::default(),
            store: store.clone(),
            connector: Arc::new(DemoConnector::default()),
            covers: Arc::new(DemoCovers),
            remote: crate::remote::Remote::default(),
        };
        let config = FakeServer::config_for_demo();

        login(&ctx, config.clone(), "secret".into()).await.unwrap();

        assert_eq!(store.credential(&config.id).unwrap(), None);
        let Some(Command::Connected { credential, .. }) = received.recv().await else {
            panic!("expected Connected");
        };
        assert_eq!(credential, Some(demo_credential()));
        assert!(ctx.session.get().is_some());
    }

    #[test]
    fn urls_are_normalized() {
        assert_eq!(
            normalize_url(" music.example.com/ ").as_deref(),
            Some("https://music.example.com")
        );
        assert_eq!(
            normalize_url("http://10.0.0.2:4533").as_deref(),
            Some("http://10.0.0.2:4533")
        );
        assert_eq!(normalize_url("  "), None);
        assert_eq!(host_of("http://10.0.0.2:4533/nd"), "10.0.0.2:4533");
    }
}
