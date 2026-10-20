use std::sync::Arc;

use super::{AppWindow, Context};

mod callbacks;
mod choices;
mod display;
mod language;
mod loading;
mod lyrics_sources;
mod servers;
mod view;
mod youtube;
mod youtube_link;

#[cfg(test)]
mod controller_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod view_tests;

pub fn install(window: &AppWindow, context: &Context) {
    callbacks::install_with(window, context, &Arc::new(view::Shared::default()));
}
