use std::process::{Command, Stdio};

use crate::{PlatformError, Result};

#[cfg(target_os = "macos")]
const OPENER: &str = "open";
#[cfg(not(target_os = "macos"))]
const OPENER: &str = "xdg-open";

const WEB_SCHEMES: [&str; 2] = ["https://", "http://"];

fn is_web_link(url: &str) -> bool {
    WEB_SCHEMES.iter().any(|scheme| url.starts_with(scheme))
}

pub fn open_url(url: &str) -> Result<()> {
    if !is_web_link(url) {
        return Err(PlatformError::OpenUrl(
            format!("{url} is not a web link").into(),
        ));
    }
    let status = Command::new(OPENER)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| PlatformError::OpenUrl(Box::new(error)))?;
    match status.success() {
        true => Ok(()),
        false => Err(PlatformError::OpenUrl(
            format!("{OPENER} exited with {status}").into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_link_that_is_not_on_the_web_it_is_refused() {
        for url in ["file:///etc/hosts", "-a Calculator", "javascript:void(0)"] {
            assert!(
                matches!(open_url(url), Err(PlatformError::OpenUrl(_))),
                "{url}"
            );
        }
    }

    #[test]
    fn given_an_http_or_https_link_it_is_a_web_link() {
        assert!(is_web_link("https://music.youtube.example/watch?v=video-1"));
        assert!(is_web_link("http://navidrome.example"));
        assert!(!is_web_link("httpsx://navidrome.example"));
    }
}
