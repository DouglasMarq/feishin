use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CookieBrowser {
    Firefox,
    Chrome,
    Chromium,
    Brave,
    Vivaldi,
    Edge,
    Opera,
    Whale,
}

impl CookieBrowser {
    pub const ALL: [Self; 8] = [
        Self::Firefox,
        Self::Chrome,
        Self::Chromium,
        Self::Brave,
        Self::Vivaldi,
        Self::Edge,
        Self::Opera,
        Self::Whale,
    ];

    pub fn yt_dlp_name(self) -> &'static str {
        match self {
            Self::Firefox => "firefox",
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Brave => "brave",
            Self::Vivaldi => "vivaldi",
            Self::Edge => "edge",
            Self::Opera => "opera",
            Self::Whale => "whale",
        }
    }

    pub fn from_yt_dlp_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|browser| browser.yt_dlp_name().eq_ignore_ascii_case(name))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Firefox => "Firefox",
            Self::Chrome => "Chrome",
            Self::Chromium => "Chromium",
            Self::Brave => "Brave",
            Self::Vivaldi => "Vivaldi",
            Self::Edge => "Edge",
            Self::Opera => "Opera",
            Self::Whale => "Whale",
        }
    }
}
