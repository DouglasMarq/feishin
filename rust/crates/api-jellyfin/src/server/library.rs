use feishin_api::Result;
use feishin_core::model::{Genre, MusicFolder, SortOrder};
use reqwest::Method;

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::GENRE_FIELDS;
use crate::normalize;
use crate::query_params::QueryParams;
use crate::sorting::{SORT_BY_NAME, sort_order};

const MUSIC_COLLECTION_TYPE: &str = "music";

impl JellyfinServer {
    pub(super) async fn ping_system_info(&self) -> Result<()> {
        self.execute(self.request(Method::GET, "system/info")).await
    }

    pub(super) async fn all_genres(&self) -> Result<Vec<Genre>> {
        let params = QueryParams::default()
            .set("EnableTotalRecordCount", true)
            .fields(GENRE_FIELDS)
            .set("Recursive", true)
            .set("SortBy", SORT_BY_NAME)
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Genre> = self.get("musicgenres", &params).await?;
        Ok(list.items.into_iter().map(normalize::genre).collect())
    }

    pub(super) async fn music_libraries(&self) -> Result<Vec<MusicFolder>> {
        let list: dto::ItemList<dto::MusicFolder> = self
            .get(&self.items_path(), &QueryParams::default())
            .await?;
        Ok(list
            .items
            .into_iter()
            .filter(|folder| folder.collection_type.as_deref() == Some(MUSIC_COLLECTION_TYPE))
            .map(normalize::music_folder)
            .collect())
    }
}
