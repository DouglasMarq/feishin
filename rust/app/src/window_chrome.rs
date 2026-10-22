use anyhow::{Context, Result};
use slint::ComponentHandle;

use crate::ui::{AppState, AppWindow};

const HAS_INSET_TITLE_BAR: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
fn select_backend() -> Result<()> {
    use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attributes| {
            attributes
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true)
        })
        .select()
        .context("selecting the window backend")
}

#[cfg(not(target_os = "macos"))]
fn select_backend() -> Result<()> {
    Ok(())
}

pub fn create_window() -> Result<AppWindow> {
    select_backend()?;
    let window = AppWindow::new().context("creating the window")?;
    window
        .global::<AppState>()
        .set_has_inset_title_bar(HAS_INSET_TITLE_BAR);
    Ok(window)
}
