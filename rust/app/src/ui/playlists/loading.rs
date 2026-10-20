use feishin_core::model::Playlist;
use slint::{ComponentHandle, Model};

use super::catalog::fetch_all;
use super::model::PlaylistsModel;
use crate::ui::images::to_image;
use crate::ui::{AppState, AppWindow, Context, PlaylistsState, Route, on_ui, show_status};

const PLAYLIST_COVER_SIZE: u32 = 96;

fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&PlaylistsModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<PlaylistsState>().get_rows();
    rows.as_any().downcast_ref::<PlaylistsModel>().map(action)
}

pub fn load(window: &AppWindow, context: &Context) {
    let Some(server) = context.session.get() else {
        return;
    };
    let Some(generation) = with_model(window, PlaylistsModel::begin) else {
        return;
    };
    window.global::<PlaylistsState>().set_is_loading(true);
    let weak = window.as_weak();
    context.runtime.spawn(async move {
        let result = fetch_all(server.as_ref()).await;
        on_ui(&weak, move |window| {
            apply_loaded(window, generation, result);
        });
    });
}

fn apply_loaded(window: &AppWindow, generation: u64, result: feishin_api::Result<Vec<Playlist>>) {
    let playlists = result
        .inspect_err(|error| show_status(window, &tr!("Could not load playlists: {}", error)))
        .unwrap_or_default();
    if with_model(window, |model| model.set_playlists(generation, &playlists)) == Some(true) {
        window.global::<PlaylistsState>().set_is_loading(false);
    }
}

pub fn refresh(window: &AppWindow) {
    if window.global::<AppState>().get_route() == Route::Playlists {
        window.global::<PlaylistsState>().invoke_load("".into());
    }
}

pub fn fetch_cover(weak: slint::Weak<AppWindow>, context: &Context, cover_id: String) {
    let Some(server) = context.session.get() else {
        return;
    };
    let covers = context.covers.clone();
    context.runtime.spawn(async move {
        match covers
            .cover(server.as_ref(), &cover_id, PLAYLIST_COVER_SIZE)
            .await
        {
            Ok(cover) => on_ui(&weak, move |window| {
                let image = to_image(&cover);
                with_model(window, |model| model.apply_cover(cover_id, image));
            }),
            Err(error) => tracing::debug!("playlist cover {cover_id}: {error:#}"),
        }
    });
}
