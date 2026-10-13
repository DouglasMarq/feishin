use feishin_core::model::{SongListQuery, SongSort, SortOrder};
use slint::{ComponentHandle, Model};

use super::columns::TrackColumn;
use super::model::TracksModel;
use super::with_model;
use crate::ui::{AppWindow, TracksState};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    pub favorite_only: bool,
    pub genre_id: Option<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
}

impl Filters {
    pub fn apply(&self, query: SongListQuery) -> SongListQuery {
        let (year_from, year_to) = match (self.year_from, self.year_to) {
            (Some(from), Some(to)) if from > to => (Some(to), Some(from)),
            years => years,
        };
        SongListQuery {
            favorite: self.favorite_only.then_some(true),
            genre_ids: self.genre_id.iter().cloned().collect(),
            year_from,
            year_to,
            ..query
        }
    }
}

pub fn parse_year(text: &str) -> Option<i32> {
    text.trim()
        .parse()
        .ok()
        .filter(|year| (1..=9999).contains(year))
}

pub fn sort_for(column: &str) -> SongSort {
    TrackColumn::from_id(column).map_or(SongSort::Name, TrackColumn::sort)
}

pub(super) fn sort_order(descending: bool) -> SortOrder {
    match descending {
        true => SortOrder::Desc,
        false => SortOrder::Asc,
    }
}

fn selected_genre_id(state: &TracksState<'_>) -> Option<String> {
    usize::try_from(state.get_genre_index() - 1)
        .ok()
        .and_then(|genre_position| state.get_genre_ids().row_data(genre_position))
        .map(|id| id.to_string())
}

fn filters_from_window(window: &AppWindow) -> Filters {
    let state = window.global::<TracksState>();
    Filters {
        favorite_only: state.get_filter_favorite(),
        genre_id: selected_genre_id(&state),
        year_from: parse_year(&state.get_year_from()),
        year_to: parse_year(&state.get_year_to()),
    }
}

pub(super) fn current_query(window: &AppWindow) -> Option<SongListQuery> {
    let query = with_model(window, TracksModel::query)?;
    Some(filters_from_window(window).apply(query))
}

pub(super) fn reload(window: &AppWindow, query: SongListQuery) {
    let state = window.global::<TracksState>();
    state.set_loading(true);
    state.set_selected_count(0);
    with_model(window, |model| model.reset(query));
}

pub fn refresh(window: &AppWindow) {
    window.global::<TracksState>().invoke_load_genres();
    if let Some(query) = current_query(window) {
        reload(window, query);
    }
}
