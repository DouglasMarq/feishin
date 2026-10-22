use std::cell::RefCell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString, VecModel};

use crate::ui::{AppState, AppWindow, Page, PlayerState, Route, TracksState};

const WINDOW_SIZE: LogicalSize = LogicalSize::new(1400.0, 900.0);
const POPUP_GAP: f32 = 4.0;

fn open_tracks() -> AppWindow {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap();
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.set_route(Route::Tracks);
    let genres: Vec<SharedString> = ["All genres", "Rock", "Jazz"]
        .map(SharedString::from)
        .into();
    window
        .global::<TracksState>()
        .set_genres(ModelRc::new(VecModel::from(genres)));
    window.window().set_size(WINDOW_SIZE);
    window.show().unwrap();
    window
}

fn element(window: &AppWindow, id: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(window, id)
        .next()
        .unwrap()
}

fn click_at(window: &AppWindow, position: LogicalPosition) {
    let button = PointerEventButton::Left;
    let window = window.window();
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed { position, button });
    window.dispatch_event(WindowEvent::PointerReleased { position, button });
}

fn click(window: &AppWindow, element: &ElementHandle) {
    let position = element.absolute_position();
    let size = element.size();
    click_at(
        window,
        LogicalPosition::new(
            position.x + size.width / 2.0,
            position.y + size.height / 2.0,
        ),
    );
}

fn type_text(window: &AppWindow, text: &str) {
    for character in text.chars() {
        let text = SharedString::from(character.to_string());
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }
}

#[test]
fn given_a_picked_genre_the_picker_selects_it() {
    let window = open_tracks();
    let changes = Rc::new(RefCell::new(0));
    let counted = changes.clone();
    window
        .global::<TracksState>()
        .on_filters_changed(move |_| *counted.borrow_mut() += 1);
    let picker = element(&window, "TooltipComboBox::touch");
    click(&window, &picker);
    let jazz = ElementHandle::find_by_element_id(&window, "TooltipComboBox::option-touch")
        .nth(2)
        .unwrap();
    let picker_position = picker.absolute_position();
    let row_position = jazz.absolute_position();
    click_at(
        &window,
        LogicalPosition::new(
            picker_position.x + row_position.x + jazz.size().width / 2.0,
            picker_position.y
                + picker.size().height
                + POPUP_GAP
                + row_position.y
                + jazz.size().height / 2.0,
        ),
    );
    assert_eq!(window.global::<TracksState>().get_genre_index(), 2);
    assert_eq!(*changes.borrow(), 1);
}

#[test]
fn given_a_cleared_search_the_text_is_empty() {
    let window = open_tracks();
    let edits = Rc::new(RefCell::new(Vec::new()));
    let recorded = edits.clone();
    window
        .global::<TracksState>()
        .on_search_edited(move |text| recorded.borrow_mut().push(text.to_string()));
    click(&window, &element(&window, "SearchField::input"));
    type_text(&window, "jazz");
    assert_eq!(window.global::<TracksState>().get_search_text(), "jazz");
    click(&window, &element(&window, "SearchField::clear-touch"));
    assert_eq!(window.global::<TracksState>().get_search_text(), "");
    assert_eq!(edits.borrow().last().map(String::as_str), Some(""));
    assert_eq!(edits.borrow().first().map(String::as_str), Some("j"));
}

#[test]
fn given_the_tracks_page_opens_space_still_toggles_playback() {
    let window = open_tracks();
    let toggles = Rc::new(RefCell::new(0));
    let counted = toggles.clone();
    window
        .global::<PlayerState>()
        .on_toggle_pause(move || *counted.borrow_mut() += 1);
    type_text(&window, " ");
    assert_eq!(*toggles.borrow(), 1);
}
