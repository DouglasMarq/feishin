use i_slint_backend_testing::ElementHandle;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, SharedString};

use super::{AppearancePage, open_with};
use crate::ui::{AppState, Page, Route};
use feishin_storage::appearance::{AccentColor, AppearanceSettings, ThemeId};

const TALL_WINDOW: LogicalSize = LogicalSize::new(1400.0, 4000.0);
const POPUP_GAP: f32 = 4.0;
const DRACULA_ROW: usize = 3;
const GREEN_SWATCH: usize = 2;

fn open_settings(settings: &AppearanceSettings) -> AppearancePage {
    let page = open_with(settings);
    let app = page.window.global::<AppState>();
    app.set_page(Page::Main);
    app.set_route(Route::Settings);
    page.window.window().set_size(TALL_WINDOW);
    page.window.show().unwrap();
    page
}

fn elements(page: &AppearancePage, id: &str) -> Vec<ElementHandle> {
    ElementHandle::find_by_element_id(&page.window, id).collect()
}

fn click_at(page: &AppearancePage, position: LogicalPosition) {
    let button = PointerEventButton::Left;
    let window = page.window.window();
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed { position, button });
    window.dispatch_event(WindowEvent::PointerReleased { position, button });
}

fn click(page: &AppearancePage, element: &ElementHandle) {
    let position = element.absolute_position();
    let size = element.size();
    click_at(
        page,
        LogicalPosition::new(
            position.x + size.width / 2.0,
            position.y + size.height / 2.0,
        ),
    );
}

fn click_popup_row(page: &AppearancePage, picker: &ElementHandle, row: &ElementHandle) {
    let picker_position = picker.absolute_position();
    let row_position = row.absolute_position();
    let row_size = row.size();
    click_at(
        page,
        LogicalPosition::new(
            picker_position.x + row_position.x + row_size.width / 2.0,
            picker_position.y
                + picker.size().height
                + POPUP_GAP
                + row_position.y
                + row_size.height / 2.0,
        ),
    );
}

fn type_text(page: &AppearancePage, text: &str) {
    let window = page.window.window();
    for character in text.chars() {
        let text = SharedString::from(character.to_string());
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
    }
}

fn press_enter(page: &AppearancePage) {
    let text = SharedString::from(Key::Return);
    let window = page.window.window();
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}

#[test]
fn clicking_the_follow_system_switch_turns_it_on_and_saves() {
    let mut page = open_settings(&AppearanceSettings::default());
    let switches = elements(&page, "ToggleSwitch::toggle");
    click(&page, &switches[0]);
    assert!(page.state().get_is_following_system());
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].is_following_system);
}

#[test]
fn picking_a_theme_from_the_list_applies_and_saves_it() {
    let mut page = open_settings(&AppearanceSettings::default());
    let picker = elements(&page, "ThemePicker::touch").remove(0);
    click(&page, &picker);
    let rows = elements(&page, "ThemeListItem::touch");
    click_popup_row(&page, &picker, &rows[DRACULA_ROW]);
    assert_eq!(
        page.theme().get_palette_index(),
        i32::try_from(DRACULA_ROW).unwrap()
    );
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].theme, ThemeId::new("dracula"));
}

#[test]
fn clicking_an_accent_swatch_uses_it() {
    let settings = AppearanceSettings {
        is_using_theme_accent: false,
        ..AppearanceSettings::default()
    };
    let mut page = open_settings(&settings);
    let swatches = elements(&page, "AccentSwatch::touch");
    click(&page, &swatches[GREEN_SWATCH]);
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].accent, AccentColor::new(0x1d, 0xb9, 0x54));
    assert_eq!(page.state().get_accent_text(), "#1db954");
}

#[test]
fn typing_an_accent_and_pressing_enter_uses_it() {
    let settings = AppearanceSettings {
        is_using_theme_accent: false,
        ..AppearanceSettings::default()
    };
    let mut page = open_settings(&settings);
    let input = elements(&page, "AccentInput::input").remove(0);
    click(&page, &input);
    page.state().set_accent_text(SharedString::new());
    type_text(&page, "#aa6ed8");
    press_enter(&page);
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].accent, AccentColor::new(0xaa, 0x6e, 0xd8));
}
