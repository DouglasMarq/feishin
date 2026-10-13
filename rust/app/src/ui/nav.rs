use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::{AppState, AppWindow, Route};

/// Browser-like history for `AppState.navigate` / `AppState.back`, kept on the UI thread.
pub fn install(window: &AppWindow) {
    let history: Rc<RefCell<Vec<(Route, SharedString)>>> = Rc::default();
    let state = window.global::<AppState>();

    let weak = window.as_weak();
    let stack = Rc::clone(&history);
    state.on_navigate(move |route, id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<AppState>();
        let current = (state.get_route(), state.get_detail_id());
        if current == (route, id.clone()) {
            return;
        }
        let mut stack = stack.borrow_mut();
        stack.push(current);
        state.set_detail_id(id);
        state.set_route(route);
        state.set_can_go_back(true);
    });

    let weak = window.as_weak();
    state.on_back(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut stack = history.borrow_mut();
        let Some((route, id)) = stack.pop() else {
            return;
        };
        let state = window.global::<AppState>();
        state.set_detail_id(id);
        state.set_route(route);
        state.set_can_go_back(!stack.is_empty());
    });
}
