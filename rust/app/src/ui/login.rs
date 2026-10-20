use std::sync::Arc;

use anyhow::{Result, anyhow};
use feishin_api::{Credential, MusicServer};
use feishin_core::model::{ServerConfig, ServerType};
use feishin_storage::feishin_import::ImportedServer;
use slint::ComponentHandle;

use super::{
    AppState, AppWindow, Context, LoginState, Page, ServerKind, on_ui, sharing, time_based_id,
    tracks,
};
use crate::controller::Command;

fn server_type(kind: ServerKind) -> ServerType {
    match kind {
        ServerKind::Navidrome => ServerType::Navidrome,
        ServerKind::Subsonic => ServerType::Subsonic,
        ServerKind::Jellyfin => ServerType::Jellyfin,
    }
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

pub fn enter_session(
    context: &Context,
    server: &Arc<dyn MusicServer>,
    config: ServerConfig,
    credential: Option<Credential>,
    others: Vec<ImportedServer>,
) {
    context.session.set(Some(Arc::clone(server)));
    context.send(Command::Connected {
        config,
        credential,
        others,
    });
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
    context: &Context,
    config: ServerConfig,
    password: String,
) -> Result<Arc<dyn MusicServer>> {
    let (server, credential) = context.connector.login(config.clone(), &password).await?;
    enter_session(context, &server, config, Some(credential), Vec::new());
    Ok(server)
}

fn first_restored_session(
    context: &Context,
    imported: &[ImportedServer],
) -> Option<(ServerConfig, Arc<dyn MusicServer>)> {
    imported.iter().find_map(|server| {
        let credential = server.credential.as_ref()?;
        context
            .connector
            .restore(&server.config, credential)
            .inspect_err(|error| tracing::warn!("{}: {error:#}", server.config.name))
            .ok()
            .map(|session| (server.config.clone(), session))
    })
}

async fn import(context: &Context) -> Result<Arc<dyn MusicServer>> {
    let store = Arc::clone(&context.store);
    let imported = tokio::task::spawn_blocking(move || store.import_feishin()).await??;
    let (config, server) = first_restored_session(context, &imported).ok_or_else(|| {
        anyhow!(tr_plural!(
            "Found {n} server in Feishin but it has no usable saved login",
            "Found {n} servers in Feishin but none with a usable saved login",
            imported.len()
        ))
    })?;
    enter_session(context, &server, config, None, imported);
    Ok(server)
}

fn config_from_form(state: &LoginState<'_>, url: String) -> ServerConfig {
    ServerConfig {
        id: time_based_id(),
        name: host_of(&url),
        server_type: server_type(state.get_server_type()),
        url,
        username: state.get_username().trim().to_owned(),
    }
}

fn install_login(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let login_context = context.clone();
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
        let context = login_context.clone();
        let weak = weak.clone();
        login_context.runtime.spawn(async move {
            let result = login(&context, config, password).await;
            finish(&weak, result);
        });
    });
}

fn install_import(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let import_context = context.clone();
    window.global::<LoginState>().on_import_feishin(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<LoginState>();
        state.set_busy(true);
        state.set_error("".into());
        let context = import_context.clone();
        let weak = weak.clone();
        import_context.runtime.spawn(async move {
            let result = import(&context).await;
            finish(&weak, result);
        });
    });
}

pub fn install(window: &AppWindow, context: &Context) {
    install_login(window, context);
    install_import(window, context);
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

    #[test]
    fn given_each_server_kind_in_the_form_it_maps_to_the_same_server_type() {
        assert_eq!(server_type(ServerKind::Navidrome), ServerType::Navidrome);
        assert_eq!(server_type(ServerKind::Subsonic), ServerType::Subsonic);
        assert_eq!(server_type(ServerKind::Jellyfin), ServerType::Jellyfin);
    }

    #[tokio::test]
    async fn given_a_login_the_credential_travels_to_the_controller() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonStore::new(directory.path()));
        let (commands, mut received) = mpsc::unbounded_channel();
        let context = Context {
            runtime: tokio::runtime::Handle::current(),
            commands,
            session: Session::default(),
            store: store.clone(),
            connector: Arc::new(DemoConnector::default()),
            covers: Arc::new(DemoCovers),
            remote: crate::remote::Remote::default(),
        };
        let config = FakeServer::config_for_demo();

        login(&context, config.clone(), "secret".into())
            .await
            .unwrap();

        assert_eq!(store.credential(&config.id).unwrap(), None);
        let Some(Command::Connected { credential, .. }) = received.recv().await else {
            panic!("expected Connected");
        };
        assert_eq!(credential, Some(demo_credential()));
        assert!(context.session.get().is_some());
    }

    #[tokio::test]
    async fn given_an_entered_session_the_server_is_current_and_the_controller_learns_the_others() {
        let directory = tempfile::tempdir().unwrap();
        let (commands, mut received) = mpsc::unbounded_channel();
        let context = Context {
            runtime: tokio::runtime::Handle::current(),
            commands,
            session: Session::default(),
            store: Arc::new(JsonStore::new(directory.path())),
            connector: Arc::new(DemoConnector::default()),
            covers: Arc::new(DemoCovers),
            remote: crate::remote::Remote::default(),
        };
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::new(3));
        let config = FakeServer::config_for_demo();
        let other = ImportedServer {
            config: config.clone(),
            credential: None,
        };

        enter_session(&context, &server, config.clone(), None, vec![other.clone()]);

        assert!(
            context
                .session
                .get()
                .is_some_and(|current| current.config().id == config.id)
        );
        let Some(Command::Connected {
            config: connected,
            credential,
            others,
        }) = received.recv().await
        else {
            panic!("expected Connected");
        };
        assert_eq!((connected, credential, others), (config, None, vec![other]));
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
