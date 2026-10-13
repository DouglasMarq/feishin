use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::{AppState, AppWindow, Route};

type History = Rc<RefCell<Vec<(Route, SharedString)>>>;

fn install_navigate(window: &AppWindow, history: History) {
    let weak = window.as_weak();
    window
        .global::<AppState>()
        .on_navigate(move |route, detail_id| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let state = window.global::<AppState>();
            let current = (state.get_route(), state.get_detail_id());
            if current == (route, detail_id.clone()) {
                return;
            }
            history.borrow_mut().push(current);
            state.set_detail_id(detail_id);
            state.set_route(route);
            state.set_can_go_back(true);
        });
}

fn install_back(window: &AppWindow, history: History) {
    let weak = window.as_weak();
    window.global::<AppState>().on_back(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut stack = history.borrow_mut();
        let Some((route, detail_id)) = stack.pop() else {
            return;
        };
        let state = window.global::<AppState>();
        state.set_detail_id(detail_id);
        state.set_route(route);
        state.set_can_go_back(!stack.is_empty());
    });
}

pub fn install(window: &AppWindow) {
    let history = History::default();
    install_navigate(window, Rc::clone(&history));
    install_back(window, history);
}
