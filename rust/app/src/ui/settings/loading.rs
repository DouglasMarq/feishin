use std::sync::Arc;
use std::sync::atomic::Ordering;

use slint::ComponentHandle;
use tokio::sync::oneshot;

use super::display::show;
use super::lyrics_sources::enabled_lyrics_sources;
use super::view::{Shared, View};
use crate::controller::Command;
use crate::ui::{AppWindow, Context, on_ui};

pub(super) fn load(window: &AppWindow, context: &Context, shared: &Arc<Shared>) {
    let request = shared.generation.fetch_add(1, Ordering::Relaxed) + 1;
    let (reply, response) = oneshot::channel();
    context.send(Command::GetSettings(reply));
    let weak_window = window.as_weak();
    let shared = Arc::clone(shared);
    context.runtime.spawn(async move {
        let snapshot = match response.await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!("the controller did not answer the settings request: {error}");
                return;
            }
        };
        on_ui(&weak_window, move |window| {
            if shared.generation.load(Ordering::Relaxed) == request {
                show(window, &shared, snapshot);
            }
        });
    });
}

pub(super) fn save_integrations(context: &Context, view: &mut View) {
    view.settings.integrations.lyrics_sources = enabled_lyrics_sources(&view.lyrics);
    context.send(Command::SaveIntegrations(
        view.settings.integrations.clone(),
    ));
}
