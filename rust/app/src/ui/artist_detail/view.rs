use feishin_core::model::ArtistDetail;
use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};

use super::super::tracks::track_row;
use super::super::{AppWindow, ArtistAlbumCard, ArtistDetailState, TrackRow};
use super::biography::plain_text;
use super::current::update_current;
use super::info::{album_card, info_line};

pub fn set_album_image(window: &AppWindow, index: usize, image: Option<Image>) {
    let model = window.global::<ArtistDetailState>().get_albums();
    if let Some(mut card) = model.row_data(index) {
        card.has_image = image.is_some();
        card.image = image.unwrap_or_default();
        model.set_row_data(index, card);
    }
}

pub fn clear(window: &AppWindow) {
    let state = window.global::<ArtistDetailState>();
    state.set_error("".into());
    state.set_name("".into());
    state.set_info("".into());
    state.set_image(Image::default());
    state.set_has_image(false);
    state.set_favorite(false);
    state.set_biography("".into());
    state.set_bio_expanded(false);
    state.set_top_expanded(false);
    state.set_top_songs(ModelRc::default());
    state.set_albums(ModelRc::default());
}

fn remember_loaded(detail: ArtistDetail) {
    update_current(|current| {
        current.loaded = true;
        current.favorite = detail.artist.favorite;
        current.album_covers = detail
            .albums
            .iter()
            .map(|album| album.cover_art_id.clone())
            .collect();
        current.top_songs = detail.top_songs;
    });
}

pub fn show(window: &AppWindow, detail: ArtistDetail, image: Option<Image>) {
    let state = window.global::<ArtistDetailState>();
    state.set_loading(false);
    state.set_name(detail.artist.name.as_str().into());
    state.set_info(info_line(&detail).into());
    state.set_favorite(detail.artist.favorite);
    state.set_biography(plain_text(detail.artist.biography.as_deref().unwrap_or_default()).into());
    if let Some(image) = image {
        state.set_image(image);
        state.set_has_image(true);
    }
    let rows: Vec<TrackRow> = detail
        .top_songs
        .iter()
        .enumerate()
        .map(|(position, song)| track_row(song, position))
        .collect();
    state.set_top_songs(ModelRc::new(VecModel::from(rows)));
    let albums: Vec<ArtistAlbumCard> = detail.albums.iter().map(album_card).collect();
    remember_loaded(detail);
    state.set_albums(ModelRc::new(VecModel::from(albums)));
}
