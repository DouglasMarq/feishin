use feishin_api::{ApiError, Credential, Result};
use feishin_core::model::ServerConfig;
use reqwest::header::AUTHORIZATION;

use super::JellyfinServer;
use crate::authorization::auth_header;
use crate::dto;
use crate::transport::{base_url, decode, http_client, send};

impl JellyfinServer {
    pub async fn login(config: ServerConfig, password: &str) -> Result<(Self, Credential)> {
        let http = http_client()?;
        let base_url = base_url(&config);
        let authentication = authenticate_by_name(&http, &base_url, &config, password).await?;
        let credential = Credential::Jellyfin {
            access_token: authentication.access_token.clone(),
            user_id: authentication.user.id.clone(),
        };
        let server = Self {
            config,
            base_url,
            access_token: authentication.access_token,
            user_id: authentication.user.id,
            http,
        };
        Ok((server, credential))
    }

    pub fn with_credential(config: ServerConfig, credential: &Credential) -> Result<Self> {
        let Credential::Jellyfin {
            access_token,
            user_id,
        } = credential
        else {
            return Err(ApiError::Auth("not a Jellyfin credential".into()));
        };
        Ok(Self {
            base_url: base_url(&config),
            access_token: access_token.clone(),
            user_id: user_id.clone(),
            http: http_client()?,
            config,
        })
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

async fn authenticate_by_name(
    http: &reqwest::Client,
    base_url: &str,
    config: &ServerConfig,
    password: &str,
) -> Result<dto::AuthResponse> {
    let request = http
        .post(format!("{base_url}/users/authenticatebyname"))
        .header(AUTHORIZATION, auth_header(&config.id, None))
        .json(&dto::AuthenticateByName {
            password,
            username: &config.username,
        });
    decode(send(request).await?).await
}
