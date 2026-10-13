const KNOWN_LYRICS_SOURCES: [(&str, &str); 4] = [
    ("server", "Server"),
    ("lrclib", "LRCLIB"),
    ("netease", "NetEase"),
    ("genius", "Genius"),
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LyricsSource {
    pub(super) id: String,
    pub(super) enabled: bool,
}

pub(super) fn lyrics_sources(enabled: &[String]) -> Vec<LyricsSource> {
    let mut sources: Vec<LyricsSource> = Vec::new();
    for id in enabled {
        push_if_missing(&mut sources, id, true);
    }
    for (id, _) in KNOWN_LYRICS_SOURCES {
        push_if_missing(&mut sources, id, false);
    }
    sources
}

fn push_if_missing(sources: &mut Vec<LyricsSource>, id: &str, enabled: bool) {
    if sources.iter().any(|source| source.id == id) {
        return;
    }
    sources.push(LyricsSource {
        id: id.to_owned(),
        enabled,
    });
}

pub(super) fn enabled_lyrics_sources(sources: &[LyricsSource]) -> Vec<String> {
    sources
        .iter()
        .filter(|source| source.enabled)
        .map(|source| source.id.clone())
        .collect()
}

pub(super) fn move_lyrics_source(sources: &mut [LyricsSource], index: usize, delta: isize) -> bool {
    match index.checked_add_signed(delta) {
        Some(target) if index < sources.len() && target < sources.len() => {
            sources.swap(index, target);
            true
        }
        _ => false,
    }
}

pub(super) fn lyrics_source_name(id: &str) -> String {
    KNOWN_LYRICS_SOURCES
        .iter()
        .find(|(known, _)| *known == id)
        .map_or_else(|| id.to_owned(), |(_, name)| (*name).to_owned())
}

#[cfg(test)]
mod tests {
    use feishin_storage::settings::IntegrationSettings;

    use super::*;

    #[test]
    fn lyrics_sources_list_enabled_first_then_the_rest() {
        let mut sources = lyrics_sources(&["lrclib".into(), "server".into(), "lrclib".into()]);
        let ids: Vec<_> = sources
            .iter()
            .map(|source| (source.id.as_str(), source.enabled))
            .collect();
        assert_eq!(
            ids,
            [
                ("lrclib", true),
                ("server", true),
                ("netease", false),
                ("genius", false)
            ]
        );
        assert!(move_lyrics_source(&mut sources, 1, -1));
        assert!(!move_lyrics_source(&mut sources, 0, -1));
        assert!(!move_lyrics_source(&mut sources, 3, 1));
        sources[3].enabled = true;
        assert_eq!(
            enabled_lyrics_sources(&sources),
            ["server", "lrclib", "genius"]
        );
        assert_eq!(lyrics_source_name("netease"), "NetEase");
        assert_eq!(lyrics_source_name("custom"), "custom");
    }

    #[test]
    fn default_lyrics_sources_survive_a_round_trip() {
        let defaults = IntegrationSettings::default().lyrics_sources;
        assert_eq!(enabled_lyrics_sources(&lyrics_sources(&defaults)), defaults);
    }
}
