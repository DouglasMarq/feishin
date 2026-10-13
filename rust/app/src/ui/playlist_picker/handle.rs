use super::dialog::open;
use super::picker::{PickedSongs, SharedPicker};
use crate::ui::tracks::ListedSongs;
use crate::ui::{AppWindow, Context};

#[derive(Clone)]
pub struct PlaylistPicker {
    shared: SharedPicker,
    ctx: Context,
}

impl PlaylistPicker {
    pub fn new(shared: SharedPicker, ctx: Context) -> Self {
        Self { shared, ctx }
    }

    pub fn open_listed(&self, window: &AppWindow, listed: ListedSongs) {
        open(window, &self.ctx, &self.shared, PickedSongs::Listed(listed));
    }
}
