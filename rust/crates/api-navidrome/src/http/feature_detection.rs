use feishin_api::Result;
use serde::Deserialize;

use crate::NavidromeServer;
use crate::features::{Features, parse_version};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ping {
    #[serde(default)]
    open_subsonic: bool,
    server_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Extensions {
    #[serde(default)]
    open_subsonic_extensions: Vec<Extension>,
}

#[derive(Debug, Deserialize)]
struct Extension {
    name: String,
    #[serde(default)]
    versions: Vec<u32>,
}

impl NavidromeServer {
    pub(crate) async fn features(&self) -> Features {
        match self
            .detected_features
            .get_or_try_init(|| self.detect_features())
            .await
        {
            Ok(features) => features.clone(),
            Err(error) => {
                tracing::warn!(%error, "Navidrome feature detection failed, assuming no features");
                Features::default()
            }
        }
    }

    async fn detect_features(&self) -> Result<Features> {
        let ping: Ping = self.subsonic("ping", &[]).await?;
        let mut features = Features {
            version: ping.server_version.as_deref().and_then(parse_version),
            ..Features::default()
        };
        if ping.open_subsonic {
            let extensions: Extensions = self.subsonic("getOpenSubsonicExtensions", &[]).await?;
            features.extensions = extensions
                .open_subsonic_extensions
                .into_iter()
                .map(|extension| (extension.name, extension.versions))
                .collect();
        }
        Ok(features)
    }
}
