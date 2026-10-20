use std::ops::RangeInclusive;

const DEFAULT_LANGUAGE: &str = "en";
const LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];
const LANGUAGE_CODE_LENGTHS: RangeInclusive<usize> = 2..=3;
const REGION_CODE_LENGTHS: RangeInclusive<usize> = 2..=2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicLocale {
    pub language: String,
    pub country: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemLocale {
    pub language: Option<String>,
    pub region: Option<String>,
}

impl SystemLocale {
    pub fn from_env() -> Self {
        let value = LOCALE_VARIABLES
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.trim().is_empty());
        value.as_deref().map(Self::parse).unwrap_or_default()
    }

    pub fn parse(posix_locale: &str) -> Self {
        let name = posix_locale
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .trim();
        let mut parts = name.split(['_', '-']);
        let language = parts
            .next()
            .filter(|language| is_letters(language, LANGUAGE_CODE_LENGTHS))
            .map(str::to_ascii_lowercase);
        let region = parts
            .next()
            .filter(|region| is_letters(region, REGION_CODE_LENGTHS))
            .map(str::to_ascii_uppercase);
        Self { language, region }
    }

    pub fn language_or_default(&self) -> String {
        self.language
            .clone()
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_owned())
    }
}

fn is_letters(text: &str, lengths: RangeInclusive<usize>) -> bool {
    lengths.contains(&text.len())
        && text
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_locales_give_language_and_region() {
        let locale = SystemLocale::parse("pt_BR.UTF-8");
        assert_eq!(locale.language.as_deref(), Some("pt"));
        assert_eq!(locale.region.as_deref(), Some("BR"));
        assert_eq!(
            SystemLocale::parse("de_DE@euro").region.as_deref(),
            Some("DE")
        );
    }

    #[test]
    fn plain_c_locales_have_no_region() {
        let locale = SystemLocale::parse("C.UTF-8");
        assert_eq!(locale.region, None);
        assert_eq!(locale.language_or_default(), "en");
        assert_eq!(SystemLocale::parse("POSIX"), SystemLocale::default());
    }
}
