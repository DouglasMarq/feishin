const MAX_IMAGE_URL_LENGTH: usize = 256;
const HTTPS_SCHEME: &str = "https://";
const CREDENTIAL_QUERY_KEYS: [&str; 7] = ["u", "p", "t", "s", "apikey", "api_key", "token"];

pub(super) fn is_shareable_image_url(url: &str) -> bool {
    has_https_scheme(url) && url.len() <= MAX_IMAGE_URL_LENGTH && !has_credential_query_key(url)
}

fn has_https_scheme(url: &str) -> bool {
    url.get(..HTTPS_SCHEME.len())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case(HTTPS_SCHEME))
}

fn query_of(url: &str) -> &str {
    let query = url.split_once('?').map_or("", |(_, query)| query);
    query.split_once('#').map_or(query, |(query, _)| query)
}

fn has_credential_query_key(url: &str) -> bool {
    query_of(url)
        .split('&')
        .filter_map(|pair| pair.split('=').next())
        .any(is_credential_key)
}

fn is_credential_key(key: &str) -> bool {
    CREDENTIAL_QUERY_KEYS
        .iter()
        .any(|credential| key.eq_ignore_ascii_case(credential))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_url_filter() {
        assert!(is_shareable_image_url("https://example.com/a.jpg"));
        assert!(is_shareable_image_url("HTTPS://example.com/a.jpg?size=300"));
        assert!(!is_shareable_image_url("http://example.com/a.jpg"));
        assert!(!is_shareable_image_url("icon"));
        assert!(!is_shareable_image_url(
            "https://nd.example.com/rest/getCoverArt?id=1&u=me&t=abc&s=salt"
        ));
        assert!(!is_shareable_image_url(
            "https://jf.example.com/Items/1/Images/Primary?api_key=secret"
        ));
        let long = format!("https://example.com/{}", "a".repeat(250));
        assert!(!is_shareable_image_url(&long));
    }
}
