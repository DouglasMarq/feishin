use feishin_api::{ApiError, Result};
use feishin_core::model::Share;

use crate::NavidromeServer;
use crate::subsonic::{SharesResponse, normalize_share};

const SHARING_DISABLED: &str = "sharing is turned off on this Navidrome server (EnableSharing)";
const NOT_IMPLEMENTED_STATUS: i32 = 501;

fn sharing_error(error: ApiError) -> ApiError {
    match error {
        ApiError::Server {
            code: NOT_IMPLEMENTED_STATUS,
            ..
        } => ApiError::Unsupported(SHARING_DISABLED),
        other => other,
    }
}

fn created_share(response: SharesResponse) -> Result<Share> {
    response
        .shares
        .unwrap_or_default()
        .share
        .into_iter()
        .find(|share| !share.url.is_empty())
        .map(normalize_share)
        .ok_or_else(|| ApiError::Decode("the server created no share link".into()))
}

impl NavidromeServer {
    pub(crate) async fn share_items(
        &self,
        item_ids: &[String],
        description: Option<&str>,
    ) -> Result<Share> {
        let mut params: Vec<(&str, String)> =
            item_ids.iter().map(|id| ("id", id.clone())).collect();
        params.extend(description.map(|text| ("description", text.to_owned())));
        let response: SharesResponse = self
            .subsonic("createShare", &params)
            .await
            .map_err(sharing_error)?;
        created_share(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_share_endpoint_means_sharing_is_disabled() {
        let not_implemented = ApiError::Server {
            code: NOT_IMPLEMENTED_STATUS,
            message: "This endpoint is not implemented".into(),
        };
        assert!(matches!(
            sharing_error(not_implemented),
            ApiError::Unsupported(SHARING_DISABLED)
        ));
        assert!(matches!(
            sharing_error(ApiError::NotFound),
            ApiError::NotFound
        ));
        assert!(matches!(
            sharing_error(ApiError::Network("down".into())),
            ApiError::Network(_)
        ));
    }
}
