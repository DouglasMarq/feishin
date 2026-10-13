use std::collections::HashMap;

const BIG_FEATURE_RELEASE_PREVIEW_TAG: &str = "pr-2709";
const BIG_FEATURE_RELEASE: ServerVersion = ServerVersion::new(0, 55, 0);
const TRACK_ARTIST_FILTER_RELEASE: ServerVersion = ServerVersion::new(0, 56, 0);
const NATIVE_PLAY_QUEUE_RELEASE: ServerVersion = ServerVersion::new(0, 57, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ServerVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl ServerVersion {
    pub(crate) const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenSubsonicExtension {
    IndexBasedQueue,
    FormPost,
    SongLyrics,
    TopSongsByArtistId,
}

impl OpenSubsonicExtension {
    const fn name(self) -> &'static str {
        match self {
            Self::IndexBasedQueue => "indexBasedQueue",
            Self::FormPost => "formPost",
            Self::SongLyrics => "songLyrics",
            Self::TopSongsByArtistId => "topSongsByArtistId",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Features {
    pub version: Option<ServerVersion>,
    pub extensions: HashMap<String, Vec<u32>>,
}

impl Features {
    fn version_at_least(&self, minimum: ServerVersion) -> bool {
        self.version.is_some_and(|version| version >= minimum)
    }

    pub fn has_big_feature_release(&self) -> bool {
        self.version_at_least(BIG_FEATURE_RELEASE)
    }

    pub fn supports_track_artist_filter(&self) -> bool {
        self.version_at_least(TRACK_ARTIST_FILTER_RELEASE)
    }

    pub fn supports_native_play_queue(&self) -> bool {
        self.version_at_least(NATIVE_PLAY_QUEUE_RELEASE)
    }

    pub fn has_extension(&self, extension: OpenSubsonicExtension) -> bool {
        self.extensions.contains_key(extension.name())
    }
}

pub(crate) fn parse_version(raw: &str) -> Option<ServerVersion> {
    if raw.contains(BIG_FEATURE_RELEASE_PREVIEW_TAG) {
        return Some(BIG_FEATURE_RELEASE);
    }

    let numeric: String = raw
        .trim()
        .trim_start_matches('v')
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    let mut parts = numeric.split('.').map(|part| part.parse::<u32>().ok());
    let major = parts.next().flatten()?;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some(ServerVersion::new(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_navidrome_versions() {
        assert_eq!(
            parse_version("0.58.0 (b2ad9d0d)"),
            Some(ServerVersion::new(0, 58, 0))
        );
        assert_eq!(parse_version("0.49.3"), Some(ServerVersion::new(0, 49, 3)));
        assert_eq!(parse_version("pr-2709"), Some(ServerVersion::new(0, 55, 0)));
        assert_eq!(parse_version("dev"), None);
    }

    #[test]
    fn feature_thresholds() {
        let old = Features {
            version: Some(ServerVersion::new(0, 54, 5)),
            ..Features::default()
        };
        assert!(!old.has_big_feature_release());
        let new = Features {
            version: Some(ServerVersion::new(0, 57, 0)),
            ..Features::default()
        };
        assert!(
            new.has_big_feature_release()
                && new.supports_track_artist_filter()
                && new.supports_native_play_queue()
        );
    }
}
