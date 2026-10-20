use std::rc::Rc;

use feishin_core::queue::{QueueView, Uid};
use slint::{ComponentHandle, Model, ModelRc};

use super::{AppWindow, Context, QueueState, clamp_i32};
use crate::controller::Command;

mod model;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod view_tests;

pub use model::{QueueModel, SharedQueueView, ViewChange, row_at};

const NO_POSITION: i32 = -1;

fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&QueueModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<QueueState>().get_rows();
    rows.as_any().downcast_ref::<QueueModel>().map(action)
}

pub fn shared_view(window: &AppWindow) -> SharedQueueView {
    with_model(window, QueueModel::shared_view).unwrap_or_else(|| {
        tracing::warn!("queue model is not installed; up next starts empty");
        SharedQueueView::default()
    })
}

fn parsed_uid(uid: &str) -> Option<Uid> {
    uid.parse()
        .inspect_err(|error| tracing::warn!("ignoring the queue row {uid:?}: {error}"))
        .ok()
}

fn install_row_actions(state: &QueueState, context: &Context) {
    let play_context = context.clone();
    state.on_play(move |uid| {
        if let Some(uid) = parsed_uid(&uid) {
            play_context.send(Command::PlayUid(uid));
        }
    });
    let remove_context = context.clone();
    state.on_remove(move |uid| {
        if let Some(uid) = parsed_uid(&uid) {
            remove_context.send(Command::Remove(vec![uid]));
        }
    });
}

fn install_move(state: &QueueState, window: &AppWindow, context: &Context) {
    let move_context = context.clone();
    let weak = window.as_weak();
    state.on_move(move |from, to| {
        let (Some(window), Ok(from), Ok(to)) =
            (weak.upgrade(), usize::try_from(from), usize::try_from(to))
        else {
            return;
        };
        let target = with_model(&window, |model| model.move_target(from, to)).flatten();
        if let Some((uid, before)) = target {
            move_context.send(Command::Move {
                uids: vec![uid],
                before,
            });
        }
    });
}

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<QueueState>();
    state.set_rows(ModelRc::from(Rc::new(QueueModel::default())));
    install_row_actions(&state, context);
    install_move(&state, window, context);
}

pub fn set_view(window: &AppWindow, view: QueueView) {
    let state = window.global::<QueueState>();
    let position = view
        .current_position()
        .map_or(NO_POSITION, |position| clamp_i32(position as u64));
    let current_uid = view
        .current()
        .map(|item| item.uid.to_string())
        .unwrap_or_default();
    if with_model(window, |model| model.set_view(view)) == Some(ViewChange::LayoutReplaced) {
        state.set_revision(state.get_revision().wrapping_add(1));
    }
    state.set_current_position(position);
    state.set_current_uid(current_uid.into());
}
