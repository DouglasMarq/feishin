use feishin_youtube::music::CHART_COUNTRIES;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::model::{DiscoverModel, with_discover};
use crate::ui::{AppWindow, DiscoverNotice, DiscoverState, clamp_i32};

pub fn country_names() -> ModelRc<SharedString> {
    let names: Vec<SharedString> = CHART_COUNTRIES
        .iter()
        .map(|country| SharedString::from(country.name))
        .collect();
    ModelRc::new(VecModel::from(names))
}

fn country_index(code: Option<&str>) -> i32 {
    let position = code.and_then(|code| {
        CHART_COUNTRIES
            .iter()
            .position(|country| country.code == code)
    });
    position.map_or(0, |position| clamp_i32(position as u64))
}

pub fn render_sections(window: &AppWindow) {
    with_discover(window, |model| render_sections_of(window, model));
}

fn render_sections_of(window: &AppWindow, model: &DiscoverModel) {
    let state = window.global::<DiscoverState>();
    let sections = model.sections.borrow();
    let request = sections.shelves_request();
    let feed = request.as_ref().and_then(|request| sections.feed(request));
    let moods: Vec<SharedString> = sections
        .moods()
        .iter()
        .map(|mood| SharedString::from(mood.title.as_str()))
        .collect();
    state.set_section(sections.section.to_slint());
    state.set_country_index(country_index(sections.chart_country.as_deref()));
    state.set_moods(ModelRc::new(VecModel::from(moods)));
    state.set_mood_index(sections.mood.map_or(-1, |mood| clamp_i32(mood as u64)));
    state.set_loading(sections.is_loading());
    state.set_error(
        sections
            .error()
            .map(|error| sections.section.load_error(error))
            .unwrap_or_default()
            .into(),
    );
    let notice = feed.map(|feed| feed.notice.clone()).unwrap_or_default();
    state.set_notice(notice.message().into());
    state.set_notice_kind(match (notice.is_warning(), notice.message().is_empty()) {
        (true, _) => DiscoverNotice::Warning,
        (false, true) => DiscoverNotice::None,
        (false, false) => DiscoverNotice::Info,
    });
    let shelves = feed.map(|feed| feed.shelves.clone()).unwrap_or_default();
    let shown = feed.and(request);
    drop(sections);
    model.show_shelves(shown.as_ref(), &shelves);
}

pub fn render_page(window: &AppWindow) {
    with_discover(window, |model| render_page_of(window, model));
}

fn render_page_of(window: &AppWindow, model: &DiscoverModel) {
    let state = window.global::<DiscoverState>();
    let page = model.page.borrow();
    let target = page.target.as_ref();
    state.set_page_kind(
        target
            .map(|target| target.kind_label())
            .unwrap_or_default()
            .into(),
    );
    state.set_page_title(page.title.as_str().into());
    state.set_page_subtitle(page.subtitle.as_str().into());
    state.set_page_loading(page.loading);
    state.set_page_error(
        page.error
            .as_deref()
            .map(|error| tr!("Could not load this page: {}", error))
            .unwrap_or_default()
            .into(),
    );
    state.set_page_has_link(target.and_then(|target| target.web_url()).is_some());
}
