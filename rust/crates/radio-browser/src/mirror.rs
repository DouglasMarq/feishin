use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ServerEntry {
    name: String,
}

fn random_index(length: usize) -> usize {
    let random = RandomState::new().build_hasher().finish();
    usize::try_from(random % u64::try_from(length.max(1)).unwrap_or(1)).unwrap_or(0)
}

fn scheme_of(discovery_url: &str) -> &str {
    discovery_url
        .split_once("://")
        .map_or("https", |(scheme, _)| scheme)
}

fn is_host_name(name: &str) -> bool {
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | ':'))
}

pub(crate) fn pick_mirror(discovery_url: &str, servers: Vec<ServerEntry>) -> Option<String> {
    let mut names: Vec<String> = servers
        .into_iter()
        .map(|server| server.name.trim().to_owned())
        .filter(|name| !name.is_empty() && is_host_name(name))
        .collect();
    names.sort();
    names.dedup();
    let chosen = names.get(random_index(names.len()))?;
    Some(format!("{}://{chosen}", scheme_of(discovery_url)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> ServerEntry {
        ServerEntry { name: name.into() }
    }

    #[test]
    fn a_mirror_is_one_of_the_named_servers_with_the_discovery_scheme() {
        let servers = vec![
            entry("de1.api.radio-browser.info"),
            entry("de1.api.radio-browser.info"),
            entry("nl1.api.radio-browser.info"),
        ];
        let mirror = pick_mirror("https://all.api.radio-browser.info", servers).unwrap_or_default();
        assert!(
            mirror == "https://de1.api.radio-browser.info"
                || mirror == "https://nl1.api.radio-browser.info"
        );
    }

    #[test]
    fn no_named_servers_means_no_mirror() {
        assert_eq!(
            pick_mirror("https://all.api.radio-browser.info", vec![entry(" ")]),
            None
        );
    }

    #[test]
    fn names_that_are_not_host_names_are_ignored() {
        let servers = vec![
            entry("evil.example.com/path?x="),
            entry("user@evil.example.com"),
            entry("de1.api.radio-browser.info"),
        ];
        assert_eq!(
            pick_mirror("https://all.api.radio-browser.info", servers),
            Some("https://de1.api.radio-browser.info".to_owned())
        );
    }
}
