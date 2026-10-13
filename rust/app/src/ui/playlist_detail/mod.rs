mod actions;
mod binding;
mod detail;
mod editing;
mod fetch_all;
mod loading;
mod model;
mod pages;
mod playing;
mod rows;
mod selecting;
mod selection;
mod targets;
#[cfg(test)]
mod tests;
mod view;

pub use loading::refresh_if_showing;
pub use view::install;
