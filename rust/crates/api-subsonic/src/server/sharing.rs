use feishin_api::{ApiError, Result};
use feishin_core::model::Share;

use super::SubsonicServer;
use crate::client::Params;
use crate::normalize;
use crate::types::Shares;

fn created_share(response: Shares) -> Result<Share> {
    response
        .shares
        .and_then(|list| list.share)
        .and_then(|shares| shares.into_iter().find(|share| !share.url.is_empty()))
        .map(normalize::share)
        .ok_or_else(|| ApiError::Decode("the server created no share link".into()))
}

impl SubsonicServer {
    pub(super) async fn share_items(
        &self,
        item_ids: &[String],
        description: Option<&str>,
    ) -> Result<Share> {
        let params = Params::new()
            .add_each("id", item_ids)
            .add_optional("description", description);
        let response: Shares = self.fetch("createShare.view", params).await?;
        created_share(response)
    }
}
