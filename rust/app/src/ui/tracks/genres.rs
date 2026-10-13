use std::cell::RefCell;
use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::Genre;
use slint::{ComponentHandle, SharedString};

use super::string_model;
use crate::session::Session;
use crate::ui::{AppWindow, Context, TracksState, on_ui};

const ALL_GENRES: &str = "All genres";

fn set_genres(window: &AppWindow, mut genres: Vec<Genre>) {
    genres.sort_by_cached_key(|genre| genre.name.to_lowercase());
    let names: Vec<SharedString> = std::iter::once(ALL_GENRES.into())
        .chain(genres.iter().map(|genre| genre.name.as_str().into()))
        .collect();
    let ids: Vec<SharedString> = genres
        .iter()
        .map(|genre| genre.id.as_str().into())
        .collect();
    let state = window.global::<TracksState>();
    state.set_genres(string_model(names));
    state.set_genre_ids(string_model(ids));
}

fn is_still_connected_to(session: &Session, server_id: &str) -> bool {
    session
        .get()
        .is_some_and(|server| server.config().id == server_id)
}

fn spawn_genre_load(
    window: &slint::Weak<AppWindow>,
    ctx: &Context,
    server: Arc<dyn MusicServer>,
    server_id: String,
) {
    let window = window.clone();
    let session = ctx.session.clone();
    ctx.rt.spawn(async move {
        let result = server.genre_list().await;
        on_ui(&window, move |window| {
            if !is_still_connected_to(&session, &server_id) {
                return;
            }
            match result {
                Ok(genres) => set_genres(window, genres),
                Err(error) => tracing::warn!("loading genres failed: {error}"),
            }
        });
    });
}

pub(super) fn install(window: &AppWindow, ctx: &Context) {
    let requested_for: RefCell<Option<String>> = RefCell::default();
    let weak = window.as_weak();
    let ctx = ctx.clone();
    window.global::<TracksState>().on_load_genres(move || {
        let (Some(window), Some(server)) = (weak.upgrade(), ctx.session.get()) else {
            return;
        };
        let server_id = server.config().id.clone();
        if requested_for.borrow().as_deref() == Some(server_id.as_str()) {
            return;
        }
        requested_for.replace(Some(server_id.clone()));
        window.global::<TracksState>().set_genre_index(0);
        set_genres(&window, Vec::new());
        spawn_genre_load(&weak, &ctx, server, server_id);
    });
}
