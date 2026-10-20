use std::sync::Arc;

use feishin_api::MusicServer;
use slint::ComponentHandle;

use super::results::{Results, with_results};
use crate::ui::{AppWindow, Context, on_ui};

pub fn fetch<Value, Failure, Pending>(
    window: &AppWindow,
    context: &Context,
    request: impl FnOnce(Arc<dyn MusicServer>) -> Pending + Send + 'static,
    done: impl FnOnce(&AppWindow, &Results, Result<Value, Failure>) + Send + 'static,
) -> bool
where
    Pending: Future<Output = Result<Value, Failure>> + Send + 'static,
    Value: Send + 'static,
    Failure: Send + 'static,
{
    let Some(server) = context.session.get() else {
        return false;
    };
    let weak = window.as_weak();
    context.runtime.spawn(async move {
        let result = request(server).await;
        on_ui(&weak, move |window| {
            with_results(window, |results| done(window, results, result));
        });
    });
    true
}
