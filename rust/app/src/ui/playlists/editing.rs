use slint::{ComponentHandle, SharedString};

use super::loading::refresh;
use crate::ui::{AppWindow, Context, PlaylistsState, on_ui, show_status};

pub fn create_playlist(window: &AppWindow, ctx: &Context, name: &SharedString) {
    let name = name.trim().to_owned();
    let Some(server) = ctx.session.get() else {
        return;
    };
    if name.is_empty() {
        return;
    }
    window.global::<PlaylistsState>().set_busy(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server.create_playlist(&name, &[]).await;
        on_ui(&weak, move |window| {
            let state = window.global::<PlaylistsState>();
            state.set_busy(false);
            match result {
                Ok(playlist) => {
                    state.set_creating(false);
                    show_status(window, &format!("Created playlist \"{}\"", playlist.name));
                    refresh(window);
                }
                Err(error) => {
                    show_status(window, &format!("Could not create the playlist: {error}"));
                }
            }
        });
    });
}

pub fn delete_playlist(window: &AppWindow, ctx: &Context, id: SharedString) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let state = window.global::<PlaylistsState>();
    let name = state.get_delete_name().to_string();
    state.set_busy(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server.delete_playlist(&id).await;
        on_ui(&weak, move |window| {
            let state = window.global::<PlaylistsState>();
            state.set_busy(false);
            state.set_delete_id("".into());
            match result {
                Ok(()) => {
                    show_status(window, &format!("Deleted playlist \"{name}\""));
                    refresh(window);
                }
                Err(error) => {
                    show_status(window, &format!("Could not delete the playlist: {error}"));
                }
            }
        });
    });
}
