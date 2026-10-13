use std::rc::Rc;

use feishin_core::model::{Page, Paging, Song};
use slint::{ComponentHandle, Model, ModelRc};

use super::model::{FolderRowsModel, SONG_PAGE_SIZE, SongPageRequest};
use crate::ui::{AppWindow, Context, FoldersState, on_ui, show_status};

pub fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&FolderRowsModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<FoldersState>().get_rows();
    rows.as_any().downcast_ref::<FolderRowsModel>().map(action)
}

pub fn first_song_page() -> Paging {
    Paging {
        start_index: 0,
        limit: SONG_PAGE_SIZE,
    }
}

pub fn install_model(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let model = Rc::new(FolderRowsModel::new(move |request| {
        spawn_song_page_load(weak.clone(), &fetch_ctx, request);
    }));
    window
        .global::<FoldersState>()
        .set_rows(ModelRc::from(model));
}

fn spawn_song_page_load(window: slint::Weak<AppWindow>, ctx: &Context, request: SongPageRequest) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        let paging = Paging {
            start_index: request.page * SONG_PAGE_SIZE,
            limit: SONG_PAGE_SIZE,
        };
        let result = server.folder_song_page(&request.folder, paging).await;
        on_ui(&window, move |window| {
            show_song_page(window, &request, result)
        });
    });
}

fn show_song_page(
    window: &AppWindow,
    request: &SongPageRequest,
    result: feishin_api::Result<Page<Song>>,
) {
    match result {
        Ok(page) => {
            with_model(window, |model| model.apply_song_page(request, page.items));
        }
        Err(error) => {
            let current = with_model(window, |model| model.song_page_failed(request));
            if current == Some(true) {
                show_status(window, &format!("Could not load the folder: {error}"));
            }
        }
    }
}
