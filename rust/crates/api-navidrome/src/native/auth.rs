use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeAuth {
    pub token: String,
    pub subsonic_salt: String,
    pub subsonic_token: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginRequest<'credentials> {
    pub username: &'credentials str,
    pub password: &'credentials str,
}
