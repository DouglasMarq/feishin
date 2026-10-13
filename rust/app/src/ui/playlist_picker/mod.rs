mod choices;
mod dialog;
mod handle;
mod listed_requests;
mod picker;
mod requests;
mod streamed;
mod view;

pub use handle::PlaylistPicker;
pub use view::install;

#[cfg(test)]
mod tests;
