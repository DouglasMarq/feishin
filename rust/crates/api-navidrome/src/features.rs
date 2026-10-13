use std::collections::HashMap;

use serde::Deserialize;

pub(crate) const EXT_INDEX_QUEUE: &str = "indexBasedQueue";
pub(crate) const EXT_FORM_POST: &str = "formPost";
pub(crate) const EXT_SONG_LYRICS: &str = "songLyrics";
pub(crate) const EXT_TOP_SONGS_BY_ID: &str = "topSongsByArtistId";

/// Server capabilities derived from the Navidrome version and OpenSubsonic extensions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Features {
    pub version: Option<(u32, u32, u32)>,
    pub extensions: HashMap<String, Vec<u32>>,
}

impl Features {
    fn at_least(&self, min: (u32, u32, u32)) -> bool {
        self.version.is_some_and(|v| v >= min)
    }

    /// "Big feature release": tag based genres, `missing` flag, repeatable filters, roles.
    pub fn bfr(&self) -> bool {
        self.at_least((0, 55, 0))
    }

    pub fn track_artist_filter(&self) -> bool {
        self.at_least((0, 56, 0))
    }

    pub fn native_play_queue(&self) -> bool {
        self.at_least((0, 57, 0))
    }

    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.contains_key(name)
    }
}

pub(crate) fn parse_version(raw: &str) -> Option<(u32, u32, u32)> {
    if raw.contains("pr-2709") {
        return Some((0, 55, 0));
    }

    let numeric: String = raw
        .trim()
        .trim_start_matches('v')
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let mut parts = numeric.split('.').map(|p| p.parse::<u32>().ok());
    let major = parts.next().flatten()?;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ping {
    #[serde(default)]
    pub open_subsonic: bool,
    pub server_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Extensions {
    #[serde(default)]
    pub open_subsonic_extensions: Vec<Extension>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Extension {
    pub name: String,
    #[serde(default)]
    pub versions: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_navidrome_versions() {
        assert_eq!(parse_version("0.58.0 (b2ad9d0d)"), Some((0, 58, 0)));
        assert_eq!(parse_version("0.49.3"), Some((0, 49, 3)));
        assert_eq!(parse_version("pr-2709"), Some((0, 55, 0)));
        assert_eq!(parse_version("dev"), None);
    }

    #[test]
    fn feature_thresholds() {
        let old = Features {
            version: Some((0, 54, 5)),
            ..Features::default()
        };
        assert!(!old.bfr());
        let new = Features {
            version: Some((0, 57, 0)),
            ..Features::default()
        };
        assert!(new.bfr() && new.track_artist_filter() && new.native_play_queue());
    }
}
