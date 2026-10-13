use feishin_core::model::RadioStationDraft;

const MISSING_NAME: &str = "Enter a name for the station";
const INVALID_STREAM_URL: &str = "Enter a stream URL that starts with http:// or https://";

fn is_web_url(url: &str) -> bool {
    ["http://", "https://"]
        .iter()
        .any(|scheme| url.len() > scheme.len() && url.to_ascii_lowercase().starts_with(scheme))
}

pub(super) fn draft_from_form(
    name: &str,
    stream_url: &str,
    homepage_url: &str,
) -> Result<RadioStationDraft, &'static str> {
    let name = name.trim();
    let stream_url = stream_url.trim();
    let homepage_url = homepage_url.trim();
    if name.is_empty() {
        return Err(MISSING_NAME);
    }
    if !is_web_url(stream_url) {
        return Err(INVALID_STREAM_URL);
    }
    Ok(RadioStationDraft {
        name: name.to_owned(),
        stream_url: stream_url.to_owned(),
        homepage_url: (!homepage_url.is_empty()).then(|| homepage_url.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_form_becomes_a_trimmed_draft() {
        let draft = draft_from_form(" Jazz ", " HTTPS://jazz.example.com/stream ", " ");
        assert_eq!(
            draft,
            Ok(RadioStationDraft {
                name: "Jazz".into(),
                stream_url: "HTTPS://jazz.example.com/stream".into(),
                homepage_url: None,
            })
        );
    }

    #[test]
    fn names_and_web_stream_urls_are_required() {
        assert_eq!(draft_from_form("", "http://a", ""), Err(MISSING_NAME));
        assert_eq!(draft_from_form("A", "ftp://a", ""), Err(INVALID_STREAM_URL));
        assert_eq!(draft_from_form("A", "http://", ""), Err(INVALID_STREAM_URL));
    }
}
