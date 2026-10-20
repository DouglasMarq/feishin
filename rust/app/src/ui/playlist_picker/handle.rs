use super::dialog::open;
use super::picker::{PickedSongs, SharedPicker};
use crate::ui::tracks::ListedSongs;
use crate::ui::{AppWindow, Context};

#[derive(Clone)]
pub struct PlaylistPicker {
    shared: SharedPicker,
    context: Context,
}

impl PlaylistPicker {
    pub fn new(shared: SharedPicker, context: Context) -> Self {
        Self { shared, context }
    }

    pub fn open_listed(&self, window: &AppWindow, listed: ListedSongs) {
        open(
            window,
            &self.context,
            &self.shared,
            PickedSongs::Listed(listed),
        );
    }
}
