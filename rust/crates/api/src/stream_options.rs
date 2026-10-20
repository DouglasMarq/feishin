use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamOptions {
    pub max_bitrate_kbps: Option<u32>,
    pub format: Option<TranscodeFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct TranscodeFormat(String);

impl TranscodeFormat {
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let is_valid = !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric());
        is_valid.then_some(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_format_name_it_is_trimmed_and_lowercased() {
        let format = TranscodeFormat::parse(" OPUS ").unwrap();
        assert_eq!(format.as_str(), "opus");
    }

    #[test]
    fn given_a_format_with_query_characters_it_is_rejected() {
        assert_eq!(TranscodeFormat::parse("mp3&t=stolen"), None);
        assert_eq!(TranscodeFormat::parse("mp3/../x"), None);
    }

    #[test]
    fn given_an_empty_format_it_is_rejected() {
        assert_eq!(TranscodeFormat::parse("  "), None);
    }
}
