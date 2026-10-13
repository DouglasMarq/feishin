use feishin_api::{ApiError, Result};
use feishin_core::model::ItemKind;
use serde::de::IgnoredAny;

use crate::NavidromeServer;
use crate::query::QueryParams;

const MAX_RATING: u8 = 5;

impl NavidromeServer {
    pub(crate) async fn mark_favorite(
        &self,
        kind: ItemKind,
        ids: &[String],
        favorite: bool,
    ) -> Result<()> {
        let key = favorite_id_key(kind)?;
        if ids.is_empty() {
            return Ok(());
        }
        let params: QueryParams = ids.iter().map(|id| (key, id.clone())).collect();
        let endpoint = if favorite { "star" } else { "unstar" };
        self.changing_annotations(self.subsonic::<IgnoredAny>(endpoint, &params))
            .await
            .map(|_| ())
    }

    pub(crate) async fn rate_item(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()> {
        if kind == ItemKind::Playlist {
            return Err(ApiError::Unsupported("rating playlists"));
        }
        let params: QueryParams = vec![
            ("id", id.to_owned()),
            ("rating", rating.min(MAX_RATING).to_string()),
        ];
        self.changing_annotations(self.subsonic::<IgnoredAny>("setRating", &params))
            .await
            .map(|_| ())
    }

    pub(crate) async fn report_scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        let mut params: QueryParams = vec![
            ("id", song_id.to_owned()),
            ("submission", submission.to_string()),
        ];
        params.extend(timestamp_ms.map(|time| ("time", time.to_string())));
        self.subsonic::<IgnoredAny>("scrobble", &params)
            .await
            .map(|_| ())
    }
}

fn favorite_id_key(kind: ItemKind) -> Result<&'static str> {
    match kind {
        ItemKind::Song => Ok("id"),
        ItemKind::Album => Ok("albumId"),
        ItemKind::Artist => Ok("artistId"),
        ItemKind::Playlist => Err(ApiError::Unsupported("favorite playlists")),
    }
}
