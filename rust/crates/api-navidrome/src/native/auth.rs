use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeAuth {
    pub token: String,
    pub subsonic_salt: String,
    pub subsonic_token: String,
}

#[derive(Serialize)]
pub(crate) struct LoginRequest<'credentials> {
    pub username: &'credentials str,
    pub password: &'credentials str,
}
