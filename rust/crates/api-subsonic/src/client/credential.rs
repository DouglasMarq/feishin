use feishin_api::{ApiError, Result};
use md5::{Digest, Md5};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rand::seq::IndexedRandom;

const URI_COMPONENT_RESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

const SALT_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SALT_LENGTH: usize = 12;

pub fn encode_uri_component(value: &str) -> String {
    utf8_percent_encode(value, URI_COMPONENT_RESERVED).to_string()
}

pub fn random_salt() -> String {
    let mut random_generator = rand::rng();
    (0..SALT_LENGTH)
        .filter_map(|_| SALT_ALPHABET.choose(&mut random_generator))
        .map(|byte| char::from(*byte))
        .collect()
}

pub fn md5_hex(input: &str) -> String {
    Md5::digest(input.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn token_credential(username: &str, password: &str) -> String {
    let salt = random_salt();
    let token = md5_hex(&format!("{password}{salt}"));
    format!(
        "u={}&s={}&t={}",
        encode_uri_component(username),
        encode_uri_component(&salt),
        encode_uri_component(&token)
    )
}

pub fn password_credential(username: &str, password: &str) -> String {
    format!(
        "u={}&p={}",
        encode_uri_component(username),
        encode_uri_component(password)
    )
}

pub fn validate_token_or_password_credential(credential: &str) -> Result<()> {
    let keys: Vec<&str> = credential
        .split('&')
        .filter_map(|pair| pair.split_once('=').map(|(key, _)| key))
        .collect();
    let has_key = |key: &str| keys.contains(&key);
    let has_token = has_key("s") && has_key("t");
    let has_password = has_key("p");
    (has_key("u") && (has_token || has_password))
        .then_some(())
        .ok_or_else(|| ApiError::Auth("malformed subsonic credential".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(
            encode_uri_component("a b&c=d/é!*'()~"),
            "a%20b%26c%3Dd%2F%C3%A9!*'()~"
        );
    }

    #[test]
    fn md5_matches_reference() {
        assert_eq!(md5_hex("sesamec19b2d"), "26719a1196d2a940705a59634eb18eab");
    }

    #[test]
    fn salt_is_alphanumeric() {
        let salt = random_salt();
        assert_eq!(salt.len(), 12);
        assert!(
            salt.chars()
                .all(|character| character.is_ascii_alphanumeric())
        );
    }

    #[test]
    fn validates_credentials() {
        assert!(validate_token_or_password_credential("u=a&s=b&t=c").is_ok());
        assert!(validate_token_or_password_credential("u=a&p=b").is_ok());
        assert!(validate_token_or_password_credential("u=a&s=b").is_err());
        assert!(validate_token_or_password_credential("").is_err());
    }
}
