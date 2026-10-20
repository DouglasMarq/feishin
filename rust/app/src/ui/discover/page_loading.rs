use std::sync::Arc;

use slint::ComponentHandle;

use super::covers::load_page_cover;
use super::deps::DiscoverDeps;
use super::mixes::mix_page;
use super::model::with_discover;
use super::page::PageTarget;
use super::render::render_page;
use crate::discover::mixes::MixId;
use crate::ui::{AppWindow, DiscoverState, on_ui, show_status};

pub fn open_page(window: &AppWindow, deps: &DiscoverDeps, detail_id: &str) {
    let Some(target) = PageTarget::parse(detail_id) else {
        show_status(window, &tr!("This Discover page does not exist"));
        return;
    };
    let generation = with_discover(window, |model| {
        let generation = model.page.borrow_mut().open(target.clone());
        if generation.is_some() {
            model.show_page_songs();
        }
        generation
    })
    .flatten();
    if let Some(generation) = generation {
        window.global::<DiscoverState>().set_page_has_cover(false);
        match target {
            PageTarget::Mix(mix) => show_mix_page(window, deps, mix, generation),
            other => fetch_page(window, deps, other, generation),
        }
    }
    render_page(window);
}

fn show_mix_page(window: &AppWindow, deps: &DiscoverDeps, mix: MixId, generation: u64) {
    let result = mix_page(window, mix).ok_or_else(|| tr!("This mix is no longer available"));
    let applied = with_discover(window, |model| {
        model.page.borrow_mut().show_collection(generation, result)
    });
    page_loaded(window, deps, generation, applied == Some(true));
}

fn fetch_page(window: &AppWindow, deps: &DiscoverDeps, target: PageTarget, generation: u64) {
    let source = Arc::clone(&deps.source);
    let settings = deps.settings();
    let weak_window = window.as_weak();
    let cover_deps = deps.clone();
    deps.ctx.rt.spawn(async move {
        match target {
            PageTarget::Collection { browse_id, .. } => {
                let result = source.collection(browse_id, settings).await;
                let result = result.map_err(|error| error.to_string());
                on_ui(&weak_window, move |window| {
                    let applied = with_discover(window, |model| {
                        model.page.borrow_mut().show_collection(generation, result)
                    });
                    page_loaded(window, &cover_deps, generation, applied == Some(true));
                });
            }
            PageTarget::Mix(_) => {}
            PageTarget::Search(query) => {
                let result = source.search(query, settings).await;
                let result = result.map_err(|error| error.to_string());
                on_ui(&weak_window, move |window| {
                    let applied = with_discover(window, |model| {
                        model.page.borrow_mut().show_search(generation, result)
                    });
                    page_loaded(window, &cover_deps, generation, applied == Some(true));
                });
            }
        }
    });
}

fn page_loaded(window: &AppWindow, deps: &DiscoverDeps, generation: u64, applied: bool) {
    if !applied {
        return;
    }
    let loaded = with_discover(window, |model| {
        model.show_page_songs();
        let page = model.page.borrow();
        (page.error.clone(), page.thumbnail_url.clone())
    });
    let Some((error, thumbnail_url)) = loaded else {
        return;
    };
    if let Some(error) = error {
        show_status(window, &tr!("Could not load this page: {}", error));
    }
    if let Some(url) = thumbnail_url {
        load_page_cover(window, deps, generation, url);
    }
    render_page(window);
}
