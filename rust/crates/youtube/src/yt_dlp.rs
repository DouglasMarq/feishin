use std::path::PathBuf;
use std::time::Duration;

use crate::links::songs_search_url;
use crate::search::parse_search_output;
use crate::{CookieBrowser, StreamUrl, YouTubeError, YouTubeHit, is_video_id, process, watch_url};

pub const DEFAULT_PROGRAM: &str = "yt-dlp";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SEARCH_RESULTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtDlp {
    program: PathBuf,
    cookies_from_browser: Option<CookieBrowser>,
    timeout: Duration,
}

impl Default for YtDlp {
    fn default() -> Self {
        Self::new(DEFAULT_PROGRAM)
    }
}

impl YtDlp {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            cookies_from_browser: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_cookies_from(self, browser: Option<CookieBrowser>) -> Self {
        Self {
            cookies_from_browser: browser,
            ..self
        }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    pub async fn audio_stream(&self, video_id: &str) -> Result<StreamUrl, YouTubeError> {
        if !is_video_id(video_id) {
            return Err(YouTubeError::InvalidVideoId(video_id.to_owned()));
        }
        let mut arguments = self.common_arguments();
        arguments.extend(["--no-playlist", "-f", "bestaudio", "-g", "--"].map(str::to_owned));
        arguments.push(watch_url(video_id));
        let stdout = process::run(&self.program, &arguments, self.timeout).await?;
        stdout
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("https://") || line.starts_with("http://"))
            .map(|line| StreamUrl::new(line.to_owned()))
            .ok_or(YouTubeError::NoAudioUrl)
    }

    pub async fn search_songs(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<YouTubeHit>, YouTubeError> {
        let limit = limit.clamp(1, MAX_SEARCH_RESULTS);
        let mut arguments = self.common_arguments();
        arguments.extend(["-j", "--flat-playlist", "--playlist-items"].map(str::to_owned));
        arguments.push(format!("1:{limit}"));
        arguments.push("--".to_owned());
        arguments.push(songs_search_url(query));
        let stdout = process::run(&self.program, &arguments, self.timeout).await?;
        Ok(parse_search_output(&stdout))
    }

    fn common_arguments(&self) -> Vec<String> {
        let mut arguments = vec!["--no-warnings".to_owned()];
        if let Some(browser) = self.cookies_from_browser {
            arguments.push("--cookies-from-browser".to_owned());
            arguments.push(browser.yt_dlp_name().to_owned());
        }
        arguments
    }
}
