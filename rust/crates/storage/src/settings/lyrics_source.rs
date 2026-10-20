use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsSourceSetting {
    Server,
    Lrclib,
    NetEase,
    Genius,
}

impl LyricsSourceSetting {
    pub const ALL: [Self; 4] = [Self::Server, Self::Lrclib, Self::NetEase, Self::Genius];

    pub fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Lrclib => "lrclib",
            Self::NetEase => "netease",
            Self::Genius => "genius",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|source| source.name() == name)
    }
}

impl Serialize for LyricsSourceSetting {
    fn serialize<NameSerializer: Serializer>(
        &self,
        serializer: NameSerializer,
    ) -> Result<NameSerializer::Ok, NameSerializer::Error> {
        serializer.serialize_str(self.name())
    }
}

pub(super) fn known_lyrics_sources<'de, SourcesDeserializer: Deserializer<'de>>(
    deserializer: SourcesDeserializer,
) -> Result<Vec<LyricsSourceSetting>, SourcesDeserializer::Error> {
    let names = Vec::<String>::deserialize(deserializer)?;
    Ok(names
        .iter()
        .filter_map(|name| {
            let source = LyricsSourceSetting::parse(name);
            if source.is_none() {
                tracing::warn!("ignoring the unknown lyrics source {name}");
            }
            source
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_each_lyrics_source_its_name_parses_back_to_it() {
        for source in LyricsSourceSetting::ALL {
            assert_eq!(LyricsSourceSetting::parse(source.name()), Some(source));
        }
    }

    #[test]
    fn given_a_name_with_other_case_and_spaces_it_still_parses() {
        assert_eq!(
            LyricsSourceSetting::parse(" NetEase "),
            Some(LyricsSourceSetting::NetEase)
        );
    }

    #[test]
    fn given_an_unknown_name_it_parses_to_no_source() {
        assert_eq!(LyricsSourceSetting::parse("bogus"), None);
    }
}
