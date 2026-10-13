use std::fmt::Display;
use std::sync::Arc;

use feishin_api::MusicServer;
use slint::ComponentHandle;

use super::results::{Results, with_results};
use crate::ui::{AppWindow, Context, on_ui, show_status};

pub fn fetch<Value, Failure, Pending>(
    window: &AppWindow,
    ctx: &Context,
    request: impl FnOnce(Arc<dyn MusicServer>) -> Pending + Send + 'static,
    done: impl FnOnce(&AppWindow, &Results, Result<Value, Failure>) + Send + 'static,
) -> bool
where
    Pending: Future<Output = Result<Value, Failure>> + Send + 'static,
    Value: Send + 'static,
    Failure: Send + 'static,
{
    let Some(server) = ctx.session.get() else {
        return false;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = request(server).await;
        on_ui(&weak, move |window| {
            with_results(window, |results| done(window, results, result));
        });
    });
    true
}

pub fn report(window: &AppWindow, failure: &str, error: impl Display) {
    show_status(window, &format!("{failure}: {error}"));
}
