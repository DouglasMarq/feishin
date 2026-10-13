use std::sync::Arc;

use super::{AppWindow, Context};

mod callbacks;
mod choices;
mod display;
mod loading;
mod lyrics_sources;
mod servers;
mod view;

#[cfg(test)]
mod controller_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod view_tests;

pub fn install(window: &AppWindow, ctx: &Context) {
    callbacks::install_with(window, ctx, &Arc::new(view::Shared::default()));
}
