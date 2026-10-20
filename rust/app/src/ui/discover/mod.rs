mod browser;
mod callbacks;
mod card_actions;
mod cards;
mod cover_slots;
mod covers;
mod deps;
mod loading;
mod mixes;
mod model;
mod page;
mod page_actions;
mod page_loading;
mod playing;
mod render;
mod sections;
mod send_order;

#[cfg(test)]
mod headless_tests;
#[cfg(test)]
mod model_tests;

use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, ModelRc};

use super::{AppWindow, Context, DiscoverState};
use crate::discover::DiscoverSource;
use callbacks::install_callbacks;
use deps::DiscoverDeps;
use model::DiscoverModel;
use render::country_names;

pub fn install(window: &AppWindow, ctx: &Context, source: Arc<dyn DiscoverSource>) {
    let model = Rc::new(DiscoverModel::default());
    let state = window.global::<DiscoverState>();
    state.set_songs(ModelRc::from(Rc::clone(&model.songs)));
    state.set_shelves(ModelRc::from(model));
    state.set_countries(country_names());
    install_callbacks(window, &DiscoverDeps::new(ctx, source));
}
