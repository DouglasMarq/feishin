use feishin_api::{ApiError, Result};
use feishin_core::model::ItemKind;

use super::SubsonicServer;
use crate::client::Params;

const MAX_RATING: u8 = 5;

fn favorite_id_key(kind: ItemKind) -> Result<&'static str> {
    match kind {
        ItemKind::Song => Ok("id"),
        ItemKind::Album => Ok("albumId"),
        ItemKind::Artist => Ok("artistId"),
        ItemKind::Playlist => Err(ApiError::Unsupported("favorite playlists")),
    }
}

fn favorite_endpoint(favorite: bool) -> &'static str {
    match favorite {
        true => "star.view",
        false => "unstar.view",
    }
}

impl SubsonicServer {
    pub(super) async fn update_favorite(
        &self,
        kind: ItemKind,
        ids: &[String],
        favorite: bool,
    ) -> Result<()> {
        let key = favorite_id_key(kind)?;
        if ids.is_empty() {
            return Ok(());
        }
        self.send(
            favorite_endpoint(favorite),
            Params::new().add_each(key, ids),
        )
        .await
    }

    pub(super) async fn update_rating(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()> {
        if kind == ItemKind::Playlist {
            return Err(ApiError::Unsupported("rating playlists"));
        }
        let params = Params::new()
            .add("id", id)
            .add("rating", rating.min(MAX_RATING));
        self.send("setRating.view", params).await
    }

    pub(super) async fn submit_scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        let params = Params::new()
            .add("id", song_id)
            .add("submission", submission)
            .add_optional("time", timestamp_ms.filter(|_| submission));
        self.send("scrobble.view", params).await
    }
}
