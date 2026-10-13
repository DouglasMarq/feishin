use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode};

use super::fetch::spawn_request;
use super::model::TracksModel;
use super::query::{current_query, reload, sort_for, sort_order};
use super::with_model;
use crate::controller::Command;
use crate::ui::{AppWindow, Context, TracksState};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub(super) fn install_model(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let model = Rc::new(TracksModel::new(move |request| {
        spawn_request(weak.clone(), &fetch_ctx, request);
    }));
    window
        .global::<TracksState>()
        .set_rows(ModelRc::from(model));
}

fn sort_by(window: &AppWindow, column: &SharedString) {
    let state = window.global::<TracksState>();
    let descending = state.get_sort_column() == *column && !state.get_sort_desc();
    state.set_sort_column(column.clone());
    state.set_sort_desc(descending);
    if let Some(mut query) = with_model(window, TracksModel::query) {
        query.sort = sort_for(column);
        query.order = sort_order(descending);
        reload(window, query);
    }
}

pub(super) fn install_sorting(window: &AppWindow) {
    let weak = window.as_weak();
    window.global::<TracksState>().on_sort_by(move |column| {
        if let Some(window) = weak.upgrade() {
            sort_by(&window, &column);
        }
    });
}

fn search_for(window: &AppWindow, text: &str) {
    if let Some(mut query) = with_model(window, TracksModel::query) {
        let search = text.trim();
        query.search = (!search.is_empty()).then(|| search.to_owned());
        reload(window, query);
    }
}

pub(super) fn install_search(window: &AppWindow) {
    let weak = window.as_weak();
    let debounce = Rc::new(Timer::default());
    window
        .global::<TracksState>()
        .on_search_edited(move |text| {
            let weak = weak.clone();
            debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
                if let Some(window) = weak.upgrade() {
                    search_for(&window, &text);
                }
            });
        });
}

fn apply_filters(window: &AppWindow) {
    let Some(query) = current_query(window) else {
        return;
    };
    if Some(&query) != with_model(window, TracksModel::query).as_ref() {
        reload(window, query);
    }
}

pub(super) fn install_filters(window: &AppWindow) {
    let weak = window.as_weak();
    let debounce = Rc::new(Timer::default());
    window
        .global::<TracksState>()
        .on_filters_changed(move |debounced| {
            let weak = weak.clone();
            let apply = move || {
                if let Some(window) = weak.upgrade() {
                    apply_filters(&window);
                }
            };
            match debounced {
                true => debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, apply),
                false => {
                    debounce.stop();
                    apply();
                }
            }
        });
}

pub(super) fn install_play_row(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    window.global::<TracksState>().on_play_row(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let target = with_model(&window, |model| {
            model
                .song_id_at(index)
                .map(|song_id| (model.query(), song_id))
        })
        .flatten();
        if let Some((query, song_id)) = target {
            play_ctx.send(Command::PlayFromList { query, song_id });
        }
    });
}
