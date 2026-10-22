use std::sync::Arc;

use image::ImageFormat;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::{ErrorCause, MediaControlEvent, MediaIdentity, PlatformError, Result};

#[cfg(target_os = "macos")]
const ICON_PNG: &[u8] = include_bytes!("../assets/tray-template.png");
#[cfg(not(target_os = "macos"))]
const ICON_PNG: &[u8] = include_bytes!("../assets/tray-color.png");
const IS_TEMPLATE_ICON: bool = cfg!(target_os = "macos");

const PLAY_PAUSE: &str = "play-pause";
const NEXT: &str = "next";
const PREVIOUS: &str = "previous";
const STOP: &str = "stop";
const SHOW: &str = "show";
const QUIT: &str = "quit";

type EventHandler = Arc<dyn Fn(MediaControlEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayLabels {
    pub play_pause: String,
    pub next: String,
    pub previous: String,
    pub stop: String,
    pub show: String,
    pub quit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedDevices {
    pub title: String,
    pub devices: Vec<String>,
}

pub struct Tray {
    icon: TrayIcon,
    labels: TrayLabels,
}

impl Tray {
    pub fn new(
        identity: MediaIdentity<'_>,
        labels: &TrayLabels,
        on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>,
    ) -> Result<Self> {
        let on_event: EventHandler = Arc::from(on_event);
        listen_to_menu(Arc::clone(&on_event));
        listen_to_clicks(on_event);
        let icon = TrayIconBuilder::new()
            .with_id(identity.desktop_entry)
            .with_tooltip(identity.name)
            .with_icon(tray_icon_image()?)
            .with_icon_as_template(IS_TEMPLATE_ICON)
            .with_menu(Box::new(menu(labels, None)?))
            .build()
            .map_err(tray_error)?;
        Ok(Self {
            icon,
            labels: labels.clone(),
        })
    }

    pub fn show_connected(&self, connected: Option<&ConnectedDevices>) -> Result<()> {
        let menu = menu(&self.labels, connected)?;
        self.icon.set_menu(Some(Box::new(menu)));
        Ok(())
    }
}

fn event_of(menu_id: &str) -> Option<MediaControlEvent> {
    match menu_id {
        PLAY_PAUSE => Some(MediaControlEvent::Toggle),
        NEXT => Some(MediaControlEvent::Next),
        PREVIOUS => Some(MediaControlEvent::Previous),
        STOP => Some(MediaControlEvent::Stop),
        SHOW => Some(MediaControlEvent::Raise),
        QUIT => Some(MediaControlEvent::Quit),
        _ => None,
    }
}

fn listen_to_menu(on_event: EventHandler) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(event) = event_of(&event.id().0) {
            on_event(event);
        }
    }));
}

#[cfg(target_os = "macos")]
fn listen_to_clicks(_on_event: EventHandler) {}

#[cfg(not(target_os = "macos"))]
fn listen_to_clicks(on_event: EventHandler) {
    use tray_icon::{MouseButton, TrayIconEvent};
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            ..
        } = event
        {
            on_event(MediaControlEvent::Raise);
        }
    }));
}

fn connected_menu(connected: &ConnectedDevices) -> Result<Submenu> {
    let submenu = Submenu::new(&connected.title, true);
    for device in &connected.devices {
        submenu
            .append(&MenuItem::new(device, false, None))
            .map_err(tray_error)?;
    }
    Ok(submenu)
}

fn menu(labels: &TrayLabels, connected: Option<&ConnectedDevices>) -> Result<Menu> {
    let menu = Menu::new();
    if let Some(connected) = connected {
        menu.append(&connected_menu(connected)?)
            .map_err(tray_error)?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(tray_error)?;
    }
    let playback = [
        (PLAY_PAUSE, &labels.play_pause),
        (NEXT, &labels.next),
        (PREVIOUS, &labels.previous),
        (STOP, &labels.stop),
    ];
    for (id, label) in playback {
        menu.append(&MenuItem::with_id(id, label, true, None))
            .map_err(tray_error)?;
    }
    menu.append(&PredefinedMenuItem::separator())
        .map_err(tray_error)?;
    for (id, label) in [(SHOW, &labels.show), (QUIT, &labels.quit)] {
        menu.append(&MenuItem::with_id(id, label, true, None))
            .map_err(tray_error)?;
    }
    Ok(menu)
}

fn tray_icon_image() -> Result<Icon> {
    let (rgba, width, height) = decoded_icon()?;
    Icon::from_rgba(rgba, width, height).map_err(tray_error)
}

fn decoded_icon() -> Result<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(ICON_PNG, ImageFormat::Png)
        .map_err(tray_error)?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}

fn tray_error(error: impl Into<ErrorCause>) -> PlatformError {
    PlatformError::Tray(error.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_each_menu_entry_it_sends_its_media_event() {
        let expected = [
            (PLAY_PAUSE, MediaControlEvent::Toggle),
            (NEXT, MediaControlEvent::Next),
            (PREVIOUS, MediaControlEvent::Previous),
            (STOP, MediaControlEvent::Stop),
            (SHOW, MediaControlEvent::Raise),
            (QUIT, MediaControlEvent::Quit),
        ];
        for (menu_id, event) in expected {
            assert_eq!(event_of(menu_id), Some(event));
        }
    }

    #[test]
    fn given_an_unknown_menu_entry_nothing_is_sent() {
        assert_eq!(event_of("settings"), None);
    }

    #[test]
    fn given_the_bundled_icon_it_decodes_to_square_rgba() {
        let (rgba, width, height) = decoded_icon().unwrap();
        assert_eq!(width, height);
        assert_eq!(rgba.len(), (width * height * 4) as usize);
    }
}
