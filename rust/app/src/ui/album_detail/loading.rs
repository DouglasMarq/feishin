use std::sync::Arc;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::AlbumDetail;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use super::super::album_grid::credited_artists;
use super::super::player::to_image;
use super::super::{AlbumDetailArtist, AlbumDetailState, AppWindow, Context, on_ui, show_status};
use super::info::album_info;
use super::tracks::{ordered, track_rows, with_tracks};
use crate::covers::CoverProvider;

const COVER_SIZE: u32 = 440;

fn is_shown(window: &AppWindow, album_id: &SharedString) -> bool {
    window.global::<AlbumDetailState>().get_album_id() == *album_id
}

fn show(window: &AppWindow, detail: AlbumDetail) {
    let state = window.global::<AlbumDetailState>();
    let album = &detail.album;
    let artists: Vec<AlbumDetailArtist> = credited_artists(album)
        .iter()
        .map(|artist| AlbumDetailArtist {
            id: artist.id.as_str().into(),
            name: artist.name.as_str().into(),
        })
        .collect();
    state.set_name(album.name.as_str().into());
    state.set_artists(ModelRc::new(VecModel::from(artists)));
    state.set_info(album_info(&detail).into());
    state.set_favorite(album.favorite);
    state.set_rating(i32::from(album.user_rating.unwrap_or(0)));
    let songs = ordered(detail.songs);
    let rows = track_rows(&songs);
    with_tracks(window, |tracks| {
        tracks.rows.set_vec(rows);
        *tracks.songs.borrow_mut() = songs;
    });
    state.set_loading(false);
}

fn show_error(window: &AppWindow, error: &ApiError) {
    let state = window.global::<AlbumDetailState>();
    state.set_loading(false);
    state.set_error("Album not found".into());
    show_status(window, &format!("Could not load the album: {error}"));
}

fn clear(window: &AppWindow, album_id: &SharedString) {
    let state = window.global::<AlbumDetailState>();
    state.set_album_id(album_id.clone());
    state.set_loading(true);
    state.set_error(SharedString::default());
    state.set_name(SharedString::default());
    state.set_artists(ModelRc::default());
    state.set_info(SharedString::default());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_favorite(false);
    state.set_rating(0);
    with_tracks(window, |tracks| {
        tracks.rows.clear();
        tracks.songs.borrow_mut().clear();
    });
}

async fn load_cover(
    server: Arc<dyn MusicServer>,
    covers: Arc<dyn CoverProvider>,
    weak_window: Weak<AppWindow>,
    album_id: SharedString,
    cover_id: String,
) {
    match covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await {
        Ok(cover) => on_ui(&weak_window, move |window| {
            if is_shown(window, &album_id)
                && let Some(image) = to_image(&cover)
            {
                let state = window.global::<AlbumDetailState>();
                state.set_cover(image);
                state.set_has_cover(true);
            }
        }),
        Err(error) => tracing::debug!("cover {cover_id}: {error:#}"),
    }
}

pub fn load(window: &AppWindow, ctx: &Context, album_id: SharedString) {
    clear(window, &album_id);
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak_window = window.as_weak();
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        let result = server.album(&album_id).await;
        let cover_id = result
            .as_ref()
            .ok()
            .and_then(|detail| detail.album.cover_art_id.clone());
        let shown_id = album_id.clone();
        on_ui(&weak_window, move |window| {
            if !is_shown(window, &shown_id) {
                return;
            }
            match result {
                Ok(detail) => show(window, detail),
                Err(error) => show_error(window, &error),
            }
        });
        let Some(cover_id) = cover_id else {
            return;
        };
        load_cover(server, covers, weak_window, album_id, cover_id).await;
    });
}
