use std::process::{Command, Stdio};

use crate::ui::{on_ui, show_status};

use super::deps::DiscoverDeps;
use crate::ui::AppWindow;
use slint::ComponentHandle;

const OPENER: &str = "xdg-open";

pub fn open_in_browser(window: &AppWindow, deps: &DiscoverDeps, url: String) {
    let weak_window = window.as_weak();
    deps.ctx.rt.spawn_blocking(move || {
        let opened = Command::new(OPENER)
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let failure = match opened {
            Ok(status) if status.success() => return,
            Ok(status) => format!("{OPENER} exited with {status}"),
            Err(error) => error.to_string(),
        };
        tracing::warn!("opening {url} failed: {failure}");
        on_ui(&weak_window, move |window| {
            show_status(window, &format!("Could not open YouTube Music: {failure}"));
        });
    });
}
