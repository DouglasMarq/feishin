use url::Url;

const MAX_IMAGE_URL_LENGTH: usize = 256;
const HTTPS_SCHEME: &str = "https";
const CREDENTIAL_QUERY_KEYS: [&str; 7] = ["u", "p", "t", "s", "apikey", "api_key", "token"];

pub(crate) fn is_shareable_image_url(url: &str) -> bool {
    url.len() <= MAX_IMAGE_URL_LENGTH
        && Url::parse(url).is_ok_and(|parsed| parsed.scheme() == HTTPS_SCHEME)
        && is_free_of_credentials(url)
}

pub(crate) fn is_free_of_credentials(url: &str) -> bool {
    Url::parse(url)
        .is_ok_and(|parsed| !parsed.query_pairs().any(|(key, _)| is_credential_key(&key)))
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

    #[test]
    fn given_an_encoded_credential_key_the_url_is_not_shared() {
        assert!(!is_shareable_image_url(
            "https://navidrome.example/rest/getCoverArt?id=1&%74=token"
        ));
        assert!(!is_free_of_credentials(
            "http://navidrome.example/rest/getCoverArt?id=1&%74=token"
        ));
    }

    #[test]
    fn given_a_local_cover_without_credentials_it_is_free_of_credentials() {
        assert!(is_free_of_credentials(
            "http://navidrome.example/covers/1.jpg?size=300"
        ));
        assert!(is_free_of_credentials("file:///tmp/feishin/cover.jpg"));
        assert!(!is_free_of_credentials(
            "http://jellyfin.example/Items/1/Download?apiKey=token"
        ));
        assert!(!is_free_of_credentials("not a url"));
    }
}
