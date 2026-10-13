use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, Timer, TimerMode};

use super::cover_loading::load_cover;
use super::query::normalize;
use super::results::Results;
use super::searching::{search, show_more};
use super::song_commands::install_song_commands;
use crate::ui::{AppWindow, Context, SearchState};

const DEBOUNCE: Duration = Duration::from_millis(250);

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<SearchState>();
    let results = Rc::new(Results::default());
    state.set_albums(ModelRc::from(Rc::clone(&results.albums)));
    state.set_artists(ModelRc::from(Rc::clone(&results.artists)));
    state.set_songs(ModelRc::from(results));

    let debounce = Rc::new(Timer::default());
    install_edited(window, ctx, Rc::clone(&debounce));
    install_submit(window, ctx, Rc::clone(&debounce));
    install_load(window, ctx, debounce);
    install_show_more(window, ctx);
    install_need_cover(window, ctx);
    install_song_commands(window, ctx);
}

fn search_now(weak: &slint::Weak<AppWindow>, ctx: &Context, query: Option<String>, force: bool) {
    if let Some(window) = weak.upgrade() {
        search(&window, ctx, query, force);
    }
}

fn install_edited(window: &AppWindow, ctx: &Context, debounce: Rc<Timer>) {
    let weak = window.as_weak();
    let edit_ctx = ctx.clone();
    window.global::<SearchState>().on_edited(move |text| {
        let Some(query) = normalize(&text) else {
            debounce.stop();
            search_now(&weak, &edit_ctx, None, false);
            return;
        };
        let (weak, ctx) = (weak.clone(), edit_ctx.clone());
        debounce.start(TimerMode::SingleShot, DEBOUNCE, move || {
            search_now(&weak, &ctx, Some(query.clone()), false);
        });
    });
}

fn install_submit(window: &AppWindow, ctx: &Context, debounce: Rc<Timer>) {
    let weak = window.as_weak();
    let submit_ctx = ctx.clone();
    window.global::<SearchState>().on_submit(move |text| {
        debounce.stop();
        search_now(&weak, &submit_ctx, normalize(&text), true);
    });
}

fn install_load(window: &AppWindow, ctx: &Context, debounce: Rc<Timer>) {
    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    window.global::<SearchState>().on_load(move |query| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<SearchState>();
        if !query.is_empty() {
            state.set_query(query);
        }
        debounce.stop();
        search(&window, &load_ctx, normalize(&state.get_query()), false);
    });
}

fn install_show_more(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let more_ctx = ctx.clone();
    window.global::<SearchState>().on_show_more(move |kind| {
        if let Some(window) = weak.upgrade() {
            show_more(&window, &more_ctx, kind);
        }
    });
}

fn install_need_cover(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let cover_ctx = ctx.clone();
    window
        .global::<SearchState>()
        .on_need_cover(move |kind, index| {
            if let Some(window) = weak.upgrade() {
                load_cover(&window, &cover_ctx, kind, index);
            }
        });
}
