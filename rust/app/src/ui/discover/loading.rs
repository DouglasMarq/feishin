use std::sync::Arc;

use slint::ComponentHandle;

use super::deps::DiscoverDeps;
use super::model::with_discover;
use super::render::render_sections;
use super::sections::Sections;
use crate::discover::{DiscoverFeed, FeedRequest, default_chart_country};
use crate::ui::{AppWindow, on_ui, show_status};

pub fn show_sections(window: &AppWindow, deps: &DiscoverDeps) {
    let settings = deps.settings();
    let requests = with_discover(window, |model| {
        let mut sections = model.sections.borrow_mut();
        if sections.use_settings(settings) {
            model.forget_shown_shelves();
        }
        start_requests(&mut sections, deps)
    })
    .unwrap_or_default();
    for request in requests {
        fetch_feed(window, deps, request);
    }
    render_sections(window);
}

pub fn refresh_sections(window: &AppWindow, deps: &DiscoverDeps) {
    with_discover(window, |model| {
        model.sections.borrow_mut().forget_wanted();
        model.forget_shown_shelves();
    });
    show_sections(window, deps);
}

fn start_requests(
    sections: &mut Sections,
    deps: &DiscoverDeps,
) -> Vec<(FeedRequest, feishin_storage::settings::YouTubeSettings)> {
    if sections.chart_country.is_none() {
        sections.chart_country = Some(default_chart_country(&sections.settings(), &deps.system));
    }
    let settings = sections.settings();
    sections
        .wanted()
        .into_iter()
        .filter(|request| sections.begin(request))
        .map(|request| (request, settings.clone()))
        .collect()
}

fn fetch_feed(
    window: &AppWindow,
    deps: &DiscoverDeps,
    (request, settings): (FeedRequest, feishin_storage::settings::YouTubeSettings),
) {
    let source = Arc::clone(&deps.source);
    let weak_window = window.as_weak();
    deps.ctx.rt.spawn(async move {
        let result = source
            .feed(request.clone(), settings)
            .await
            .map_err(|error| error.to_string());
        on_ui(&weak_window, move |window| {
            apply_feed(window, request, result)
        });
    });
}

fn apply_feed(window: &AppWindow, request: FeedRequest, result: Result<DiscoverFeed, String>) {
    let message = match &result {
        Ok(feed) => feed.notice.is_warning().then(|| feed.notice.message()),
        Err(error) => Some(tr!("Could not load Discover: {}", error)),
    };
    let wanted = with_discover(window, |model| {
        let mut sections = model.sections.borrow_mut();
        sections.finish(request.clone(), result);
        sections.is_wanted(&request)
    });
    if let (Some(true), Some(message)) = (wanted, message) {
        show_status(window, &message);
    }
    render_sections(window);
}
