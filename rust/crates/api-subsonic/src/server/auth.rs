use feishin_api::{ApiError, Credential, Result};
use feishin_core::model::ServerConfig;

use super::SubsonicServer;
use crate::client::{
    Params, RequestEncoding, password_credential, token_credential,
    validate_token_or_password_credential,
};

impl SubsonicServer {
    pub async fn login(config: ServerConfig, password: &str) -> Result<(Self, Credential)> {
        let token = token_credential(&config.username, password);
        let token_server = Self::new(config, token)?;
        let (server, credential) = match token_server.authenticate().await {
            Ok(()) => {
                let credential = token_server.client.auth().to_owned();
                (token_server, credential)
            }
            Err(ApiError::Auth(_)) => token_server.login_with_plain_password(password).await?,
            Err(error) => return Err(error),
        };
        Ok((
            server,
            Credential::Subsonic {
                subsonic: credential,
            },
        ))
    }

    async fn login_with_plain_password(self, password: &str) -> Result<(Self, String)> {
        let legacy = password_credential(&self.config.username, password);
        let server = Self::new(self.config, legacy.clone())?;
        server.authenticate().await?;
        Ok((server, legacy))
    }

    pub fn with_credential(config: ServerConfig, credential: &Credential) -> Result<Self> {
        match credential {
            Credential::Subsonic { subsonic } => {
                validate_token_or_password_credential(subsonic)?;
                Self::new(config, subsonic.clone())
            }
            _ => Err(ApiError::Auth("expected a subsonic credential".into())),
        }
    }

    async fn authenticate(&self) -> Result<()> {
        self.client
            .call(
                "getUser.view",
                &Params::new().add("username", &self.config.username),
                RequestEncoding::QueryString,
            )
            .await
            .map(drop)
    }
}
