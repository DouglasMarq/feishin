use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Folder, FolderId, Page, Song};
use slint::{ComponentHandle, ModelRc, VecModel};

use super::binding::{first_song_page, with_model};
use super::trail::{Crumb, SharedTrail, root_name};
use crate::ui::{
    AppState, AppWindow, Context, FolderCrumb, FoldersState, Route, on_ui, show_status,
};

struct Loaded {
    generation: u64,
    folders: feishin_api::Result<Vec<Folder>>,
    songs: feishin_api::Result<Page<Song>>,
}

pub fn load(window: &AppWindow, ctx: &Context, trail: &SharedTrail, detail_id: &str) {
    if window.global::<AppState>().get_route() != Route::Folders {
        return;
    }
    let crumbs = trail.borrow_mut().path_to(detail_id);
    show_crumbs(window, detail_id, &crumbs);
    let folder = FolderId::from_detail_id(detail_id);
    let Some(generation) = with_model(window, |model| model.reset(folder.clone())) else {
        return;
    };
    let state = window.global::<FoldersState>();
    state.set_summary("".into());
    state.set_error("".into());
    let Some(server) = ctx.session.get() else {
        state.set_loading(false);
        return;
    };
    state.set_loading(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let loaded = fetch_folder(server, folder, generation).await;
        on_ui(&weak, move |window| apply_loaded(window, loaded));
    });
}

async fn fetch_folder(server: Arc<dyn MusicServer>, folder: FolderId, generation: u64) -> Loaded {
    let (folders, songs) = tokio::join!(
        server.subfolders(&folder),
        server.folder_song_page(&folder, first_song_page())
    );
    Loaded {
        generation,
        folders,
        songs,
    }
}

fn show_crumbs(window: &AppWindow, detail_id: &str, crumbs: &[Crumb]) {
    let state = window.global::<FoldersState>();
    let rows: Vec<FolderCrumb> = crumbs
        .iter()
        .map(|crumb| FolderCrumb {
            id: crumb.id.as_str().into(),
            name: crumb.name.as_str().into(),
        })
        .collect();
    let name = crumbs
        .last()
        .map_or_else(root_name, |crumb| crumb.name.clone());
    state.set_name(name.into());
    state.set_folder_id(detail_id.into());
    state.set_crumbs(ModelRc::new(VecModel::from(rows)));
}

fn apply_loaded(window: &AppWindow, loaded: Loaded) {
    if with_model(window, |model| model.is_current(loaded.generation)) != Some(true) {
        return;
    }
    let state = window.global::<FoldersState>();
    state.set_loading(false);
    let (folders, page) = match (loaded.folders, loaded.songs) {
        (Ok(folders), Ok(page)) => (folders, page),
        (Err(error), _) | (_, Err(error)) => {
            let message = tr!("Could not open the folder: {}", error);
            state.set_error(message.as_str().into());
            show_status(window, &message);
            return;
        }
    };
    let song_total = page.total_count.unwrap_or(page.items.len());
    let folder_count = folders.len();
    with_model(window, |model| {
        model.show(loaded.generation, folders, song_total, page.items)
    });
    state.set_summary(summary(folder_count, song_total).into());
}

fn summary(folder_count: usize, song_count: usize) -> String {
    [
        (folder_count > 0).then(|| tr_plural!("{n} folder", "{n} folders", folder_count)),
        (song_count > 0).then(|| tr_plural!("{n} song", "{n} songs", song_count)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

#[cfg(test)]
mod tests {
    use super::summary;

    #[test]
    fn summaries_skip_empty_counts() {
        assert_eq!(summary(400, 0), "400 folders");
        assert_eq!(summary(1, 1), "1 folder, 1 song");
        assert_eq!(summary(0, 12), "12 songs");
        assert_eq!(summary(0, 0), "");
    }
}
