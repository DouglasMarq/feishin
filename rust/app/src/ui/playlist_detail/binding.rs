use std::rc::Rc;

use feishin_core::model::{Page, Song};
use slint::{ComponentHandle, Model, ModelRc};

use super::model::{PageRequest, PlaylistSongsModel};
use super::pages::paging_for_page;
use crate::ui::{AppWindow, Context, PlaylistDetailState, on_ui, show_status};

pub fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&PlaylistSongsModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<PlaylistDetailState>().get_rows();
    rows.as_any()
        .downcast_ref::<PlaylistSongsModel>()
        .map(action)
}

pub fn install_model(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let model = Rc::new(PlaylistSongsModel::new(move |request| {
        spawn_page_load(weak.clone(), &fetch_ctx, request);
    }));
    window
        .global::<PlaylistDetailState>()
        .set_rows(ModelRc::from(model));
}

fn spawn_page_load(window: slint::Weak<AppWindow>, ctx: &Context, request: PageRequest) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        let paging = paging_for_page(request.page);
        let result = server
            .playlist_song_page(&request.playlist_id, paging)
            .await;
        on_ui(&window, move |window| show_page(window, &request, result));
    });
}

fn show_page(window: &AppWindow, request: &PageRequest, result: feishin_api::Result<Page<Song>>) {
    let (generation, page) = (request.generation, request.page);
    match result {
        Ok(songs) => {
            with_model(window, |model| {
                model.apply_page(generation, page, songs.items);
            });
        }
        Err(error) => {
            let current = with_model(window, |model| model.page_failed(generation, page));
            if current == Some(true) {
                show_status(window, &format!("Could not load the playlist: {error}"));
            }
        }
    }
}
