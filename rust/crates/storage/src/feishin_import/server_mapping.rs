use feishin_api::Credential;
use feishin_core::model::{ServerConfig, ServerType};

use super::ImportedServer;
use super::raw_store::RawServer;

struct RawCredential {
    token: Option<String>,
    nd_token: Option<String>,
    user_id: Option<String>,
}

pub(super) fn map_server(key: &str, raw: RawServer) -> Option<ImportedServer> {
    let server_type = parse_server_type(key, raw.server_type.as_deref()?)?;
    let url = normalize_url(&chosen_url(raw.prefer_remote_url, raw.remote_url, raw.url)?);
    let id = non_empty(raw.id).unwrap_or_else(|| key.to_string());
    let name = non_empty(raw.name).unwrap_or_else(|| url.clone());
    let credential = imported_credential(
        server_type,
        RawCredential {
            token: non_empty(raw.credential),
            nd_token: raw.nd_credential,
            user_id: raw.user_id,
        },
    );
    Some(ImportedServer {
        config: ServerConfig {
            id,
            name,
            server_type,
            url,
            username: raw.username.unwrap_or_default(),
        },
        credential,
    })
}

fn parse_server_type(key: &str, raw_type: &str) -> Option<ServerType> {
    match raw_type.to_ascii_lowercase().as_str() {
        "navidrome" => Some(ServerType::Navidrome),
        "subsonic" => Some(ServerType::Subsonic),
        "jellyfin" => Some(ServerType::Jellyfin),
        other => {
            tracing::warn!("skipping Feishin server {key}: unsupported type {other}");
            None
        }
    }
}

fn chosen_url(
    prefer_remote_url: Option<bool>,
    remote_url: Option<String>,
    url: Option<String>,
) -> Option<String> {
    let remote_url = remote_url.filter(|candidate| !candidate.trim().is_empty());
    match (prefer_remote_url.unwrap_or(false), remote_url) {
        (true, Some(remote)) => Some(remote),
        _ => url.filter(|candidate| !candidate.trim().is_empty()),
    }
}

fn imported_credential(server_type: ServerType, raw: RawCredential) -> Option<Credential> {
    match server_type {
        ServerType::Navidrome => raw.token.map(|subsonic| Credential::Navidrome {
            subsonic,
            nd_token: non_empty(raw.nd_token),
        }),
        ServerType::Subsonic => raw.token.map(|subsonic| Credential::Subsonic { subsonic }),
        ServerType::Jellyfin => {
            raw.token
                .zip(non_empty(raw.user_id))
                .map(|(access_token, user_id)| Credential::Jellyfin {
                    access_token,
                    user_id,
                })
        }
    }
}

fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    trimmed.strip_suffix('/').unwrap_or(trimmed).to_string()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}
