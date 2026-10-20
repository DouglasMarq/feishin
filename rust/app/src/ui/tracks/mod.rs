mod actions;
mod columns;
mod fetch;
mod genres;
mod list_callbacks;
mod listed;
mod listed_actions;
mod model;
mod query;
mod row;
mod selection;
mod selection_callbacks;
mod selection_ranges;
#[cfg(test)]
mod tests;

use feishin_storage::settings::UiSettings;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

pub use columns::columns;
pub use listed::{LISTED_CHUNK_SIZE, ListedChunks, ListedSongs};
#[cfg(test)]
pub use model::CACHED_PAGES;
pub use model::{PAGE_SIZE, Request, TracksModel};
pub use query::refresh;
pub use row::track_row;
pub use selection_ranges::SelectionRanges;

use super::playlist_picker::PlaylistPicker;
use super::{AppWindow, Context, TracksState};

fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&TracksModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<TracksState>().get_rows();
    rows.as_any().downcast_ref::<TracksModel>().map(action)
}

fn string_model(items: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(items))
}

pub fn selected_song_ids(window: &AppWindow) -> Option<Vec<String>> {
    match with_model(window, TracksModel::selected_songs)? {
        selection::SelectedSongs::Loaded(songs) => {
            Some(songs.into_iter().map(|song| song.id).collect())
        }
        selection::SelectedSongs::Listed(_) => None,
    }
}

pub fn set_current(window: &AppWindow, song_id: Option<&str>) {
    with_model(window, |model| model.set_current(song_id));
}

pub fn install(window: &AppWindow, context: &Context, ui: &UiSettings, picker: &PlaylistPicker) {
    columns::apply_columns(window, &ui.track_columns);
    list_callbacks::install_model(window, context);
    list_callbacks::install_sorting(window);
    list_callbacks::install_search(window);
    list_callbacks::install_filters(window);
    list_callbacks::install_play_row(window, context);
    genres::install(window, context);
    selection_callbacks::install(window, context, picker);
}
