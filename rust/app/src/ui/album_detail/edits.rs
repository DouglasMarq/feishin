use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::ItemKind;
use slint::{ComponentHandle, SharedString};

use super::super::{AlbumDetailState, AppWindow, Context, on_ui, show_status};

fn shown_album(window: &AppWindow, ctx: &Context) -> Option<(SharedString, Arc<dyn MusicServer>)> {
    let album_id = window.global::<AlbumDetailState>().get_album_id();
    let server = ctx.session.get().filter(|_| !album_id.is_empty())?;
    Some((album_id, server))
}

pub fn toggle_favorite(window: &AppWindow, ctx: &Context) {
    let Some((album_id, server)) = shown_album(window, ctx) else {
        return;
    };
    let state = window.global::<AlbumDetailState>();
    let favorite = !state.get_favorite();
    state.set_favorite(favorite);
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server
            .set_favorite(ItemKind::Album, &[album_id.to_string()], favorite)
            .await;
        if let Err(error) = result {
            on_ui(&weak_window, move |window| {
                let state = window.global::<AlbumDetailState>();
                if state.get_album_id() == album_id {
                    state.set_favorite(!favorite);
                }
                show_status(window, &format!("Could not update favorite: {error}"));
            });
        }
    });
}

pub fn set_rating(window: &AppWindow, ctx: &Context, rating: i32) {
    let Some((album_id, server)) = shown_album(window, ctx) else {
        return;
    };
    let state = window.global::<AlbumDetailState>();
    let previous = state.get_rating();
    let rating = rating.clamp(0, 5);
    state.set_rating(rating);
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server
            .set_rating(ItemKind::Album, &album_id, rating as u8)
            .await;
        if let Err(error) = result {
            on_ui(&weak_window, move |window| {
                let state = window.global::<AlbumDetailState>();
                if state.get_album_id() == album_id {
                    state.set_rating(previous);
                }
                show_status(window, &format!("Could not update rating: {error}"));
            });
        }
    });
}
