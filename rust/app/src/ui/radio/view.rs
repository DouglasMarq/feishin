use std::sync::{Arc, Mutex, PoisonError};

use slint::{ComponentHandle, ModelRc, VecModel};

use super::model::RadioModel;
use crate::radio_directory::StationDirectory;
use crate::ui::{AppWindow, Context, RadioSource, RadioState};

#[derive(Clone)]
pub(super) struct RadioView {
    pub(super) context: Context,
    pub(super) window: slint::Weak<AppWindow>,
    pub(super) directory: Arc<dyn StationDirectory>,
    model: Arc<Mutex<RadioModel>>,
}

impl RadioView {
    pub(super) fn with_model<Output>(
        &self,
        action: impl FnOnce(&mut RadioModel) -> Output,
    ) -> Output {
        let mut model = self.model.lock().unwrap_or_else(PoisonError::into_inner);
        action(&mut model)
    }

    pub(super) fn render(&self, window: &AppWindow) {
        let state = window.global::<RadioState>();
        let (stations, results, can_manage, has_more) = self.with_model(|model| {
            (
                model.station_rows(),
                model.result_rows(),
                model.can_manage,
                model.has_more,
            )
        });
        state.set_stations(ModelRc::new(VecModel::from(stations)));
        state.set_results(ModelRc::new(VecModel::from(results)));
        state.set_can_manage(can_manage);
        state.set_has_more(has_more);
    }

    pub(super) fn on_window(&self, action: impl FnOnce(&AppWindow) + Send + 'static) {
        crate::ui::on_ui(&self.window, action);
    }
}

pub fn install(window: &AppWindow, context: &Context) {
    install_with(window, context, context.connector.station_directory());
}

pub(super) fn install_with(
    window: &AppWindow,
    context: &Context,
    directory: Arc<dyn StationDirectory>,
) -> RadioView {
    let view = RadioView {
        context: context.clone(),
        window: window.as_weak(),
        directory,
        model: Arc::default(),
    };
    install_loading(window, &view);
    install_playback(window, &view);
    install_saving(window, &view);
    view
}

fn install_loading(window: &AppWindow, view: &RadioView) {
    let state = window.global::<RadioState>();
    let loading = view.clone();
    state.on_load(move || loading.load());
    let searching = view.clone();
    state.on_search(move || searching.search());
    let paging = view.clone();
    state.on_load_more(move || paging.load_more());
}

fn install_playback(window: &AppWindow, view: &RadioView) {
    let state = window.global::<RadioState>();
    let playing = view.clone();
    state.on_play(move |source, id| playing.play(source, &id));
    let stopping = view.clone();
    state.on_stop(move || stopping.stop());
}

fn install_saving(window: &AppWindow, view: &RadioView) {
    let state = window.global::<RadioState>();
    let favorites = view.clone();
    state.on_toggle_favorite(move |source, id| favorites.toggle_favorite(source, &id));
    let uploading = view.clone();
    state.on_save_to_server(move |source, id| uploading.save_to_server(source, &id));
    let editing = view.clone();
    state.on_edit(move |source, id| editing.edit(source, &id));
    let deleting = view.clone();
    state.on_delete(move |id| deleting.delete(&id));
    let saving = view.clone();
    state.on_save_edit(move |target: RadioSource| saving.save_edit(target));
}
