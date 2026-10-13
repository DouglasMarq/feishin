use std::sync::Arc;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::ArtistDetail;
use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, Image};

use super::super::player::to_image;
use super::super::{AppWindow, ArtistDetailState, Context, on_ui};
use super::current::{begin, is_current, update_current};
use super::view::{clear, set_album_image, show};
use crate::covers::CoverProvider;

const IMAGE_SIZE: u32 = 360;
const ALBUM_COVER_SIZE: u32 = 200;

async fn fetch_artist_image(
    server: &dyn MusicServer,
    covers: &dyn CoverProvider,
    detail: &Result<ArtistDetail, ApiError>,
) -> Option<Arc<DecodedImage>> {
    let cover_id = detail.as_ref().ok()?.artist.cover_art_id.clone()?;
    covers
        .cover(server, &cover_id, IMAGE_SIZE)
        .await
        .inspect_err(|error| tracing::debug!("artist image {cover_id}: {error:#}"))
        .ok()
}

fn show_result(
    window: &AppWindow,
    detail: Result<ArtistDetail, ApiError>,
    image: Option<Arc<DecodedImage>>,
) {
    match detail {
        Ok(detail) => {
            let image = image.as_deref().and_then(to_image);
            show(window, detail, image);
        }
        Err(error) => {
            update_current(|current| current.id.clear());
            let state = window.global::<ArtistDetailState>();
            state.set_loading(false);
            state.set_error(format!("Could not load the artist: {error}").into());
        }
    }
}

pub fn load(window: &AppWindow, ctx: &Context, artist_id: &str) {
    let Some(generation) = begin(artist_id) else {
        return;
    };
    clear(window);
    window.global::<ArtistDetailState>().set_loading(true);
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak_window = window.as_weak();
    let covers = ctx.covers.clone();
    let artist_id = artist_id.to_owned();
    ctx.rt.spawn(async move {
        let detail = server.artist(&artist_id).await;
        let image = fetch_artist_image(server.as_ref(), covers.as_ref(), &detail).await;
        on_ui(&weak_window, move |window| {
            if is_current(generation) {
                show_result(window, detail, image);
            }
        });
    });
}

fn claim_album_cover(index: usize) -> Option<(u64, String)> {
    update_current(|current| {
        let cover_id = current.album_covers.get(index).cloned().flatten()?;
        current
            .covers
            .begin(index)
            .then_some((current.generation, cover_id))
    })
}

pub fn load_album_cover(window: &AppWindow, ctx: &Context, index: usize) {
    let request = claim_album_cover(index);
    let (Some((generation, cover_id)), Some(server)) = (request, ctx.session.get()) else {
        return;
    };
    let weak_window = window.as_weak();
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        let cover = covers
            .cover(server.as_ref(), &cover_id, ALBUM_COVER_SIZE)
            .await;
        if let Err(error) = &cover {
            tracing::debug!("album cover {cover_id}: {error:#}");
        }
        on_ui(&weak_window, move |window| {
            let image = cover.ok().and_then(|cover| to_image(&cover));
            apply_album_cover(window, generation, index, image);
        });
    });
}

fn apply_album_cover(window: &AppWindow, generation: u64, index: usize, image: Option<Image>) {
    let evicted = update_current(|current| {
        if current.generation != generation {
            return None;
        }
        match &image {
            Some(image) => Some(current.covers.insert(index, image.clone())),
            None => {
                current.covers.failed(index);
                None
            }
        }
    });
    let Some(evicted) = evicted else {
        return;
    };
    for evicted_index in evicted {
        set_album_image(window, evicted_index, None);
    }
    set_album_image(window, index, image);
}
