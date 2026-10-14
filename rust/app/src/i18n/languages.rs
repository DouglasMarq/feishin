#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralRule {
    OneOther,
    ZeroAndOneSingular,
    SingleForm,
    Czech,
    Latvian,
    Polish,
    Romanian,
    EastSlavic,
    Slovenian,
    Arabic,
}

impl PluralRule {
    #[cfg(test)]
    pub fn form_count(self) -> usize {
        match self {
            Self::SingleForm => 1,
            Self::OneOther | Self::ZeroAndOneSingular => 2,
            Self::Czech | Self::Latvian | Self::Polish | Self::Romanian | Self::EastSlavic => 3,
            Self::Slovenian => 4,
            Self::Arabic => 6,
        }
    }

    pub fn form_index(self, count: u64) -> usize {
        match self {
            Self::SingleForm => 0,
            Self::OneOther => usize::from(count != 1),
            Self::ZeroAndOneSingular => usize::from(count > 1),
            Self::Czech => czech_form(count),
            Self::Latvian => latvian_form(count),
            Self::Polish => polish_form(count),
            Self::Romanian => romanian_form(count),
            Self::EastSlavic => east_slavic_form(count),
            Self::Slovenian => slovenian_form(count),
            Self::Arabic => arabic_form(count),
        }
    }
}

fn ends_in_one(count: u64) -> bool {
    count % 10 == 1 && count % 100 != 11
}

fn ends_in_two_to_four(count: u64) -> bool {
    (2..=4).contains(&(count % 10)) && !(12..=14).contains(&(count % 100))
}

fn czech_form(count: u64) -> usize {
    match count {
        1 => 0,
        2..=4 => 1,
        _ => 2,
    }
}

fn latvian_form(count: u64) -> usize {
    match (ends_in_one(count), count) {
        (true, _) => 0,
        (false, 0) => 2,
        (false, _) => 1,
    }
}

fn polish_form(count: u64) -> usize {
    match (count, ends_in_two_to_four(count)) {
        (1, _) => 0,
        (_, true) => 1,
        (_, false) => 2,
    }
}

fn romanian_form(count: u64) -> usize {
    match count {
        1 => 0,
        _ if count == 0 || (1..20).contains(&(count % 100)) => 1,
        _ => 2,
    }
}

fn east_slavic_form(count: u64) -> usize {
    match (ends_in_one(count), ends_in_two_to_four(count)) {
        (true, _) => 0,
        (false, true) => 1,
        (false, false) => 2,
    }
}

fn slovenian_form(count: u64) -> usize {
    match count % 100 {
        1 => 0,
        2 => 1,
        3 | 4 => 2,
        _ => 3,
    }
}

fn arabic_form(count: u64) -> usize {
    match (count, count % 100) {
        (0, _) => 0,
        (1, _) => 1,
        (2, _) => 2,
        (_, 3..=10) => 3,
        (_, 11..=99) => 4,
        _ => 5,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    pub plural_rule: PluralRule,
}

const fn language(code: &'static str, name: &'static str, plural_rule: PluralRule) -> Language {
    Language {
        code,
        name,
        plural_rule,
    }
}

pub const ENGLISH: Language = language("en", "English", PluralRule::OneOther);

pub const LANGUAGES: [Language; 39] = [
    ENGLISH,
    language("ar", "العربية", PluralRule::Arabic),
    language("bg", "Български", PluralRule::OneOther),
    language("ca", "Català", PluralRule::OneOther),
    language("cs", "Čeština", PluralRule::Czech),
    language("da", "Dansk", PluralRule::OneOther),
    language("de", "Deutsch", PluralRule::OneOther),
    language("es", "Español", PluralRule::OneOther),
    language("et", "Eesti", PluralRule::OneOther),
    language("eu", "Euskara", PluralRule::OneOther),
    language("fr", "Français", PluralRule::ZeroAndOneSingular),
    language("gl", "Galego", PluralRule::OneOther),
    language("id", "Bahasa Indonesia", PluralRule::SingleForm),
    language("fi", "Suomi", PluralRule::OneOther),
    language("hu", "Magyar", PluralRule::OneOther),
    language("it", "Italiano", PluralRule::OneOther),
    language("ja", "日本語", PluralRule::SingleForm),
    language("ko", "한국어", PluralRule::SingleForm),
    language("lv", "Latviešu", PluralRule::Latvian),
    language("nl", "Nederlands", PluralRule::OneOther),
    language("nb-NO", "Norsk (Bokmål)", PluralRule::OneOther),
    language("nn", "Norsk (Nynorsk)", PluralRule::OneOther),
    language("fa", "فارسی", PluralRule::ZeroAndOneSingular),
    language("pt", "Português", PluralRule::OneOther),
    language(
        "pt-BR",
        "Português (Brasil)",
        PluralRule::ZeroAndOneSingular,
    ),
    language("pl", "Polski", PluralRule::Polish),
    language("ro", "Română", PluralRule::Romanian),
    language("ru", "Русский", PluralRule::EastSlavic),
    language("sk", "Slovenčina", PluralRule::Czech),
    language("sl", "Slovenščina", PluralRule::Slovenian),
    language("sr", "Srpski", PluralRule::EastSlavic),
    language("sv", "Svenska", PluralRule::OneOther),
    language("ta", "தமிழ்", PluralRule::OneOther),
    language("th", "ไทย", PluralRule::SingleForm),
    language("tl", "Tagalog", PluralRule::OneOther),
    language("tr", "Türkçe", PluralRule::OneOther),
    language("uk", "Українська", PluralRule::EastSlavic),
    language("zh-Hans", "简体中文", PluralRule::SingleForm),
    language("zh-Hant", "繁體中文", PluralRule::SingleForm),
];

pub fn language_by_code(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|language| language.code == code)
}

fn base_language(locale: &str) -> &str {
    locale.split(['-', '_', '@', '.']).next().unwrap_or(locale)
}

fn chinese_variant(locale: &str) -> &'static str {
    let traditional = ["Hant", "TW", "HK", "MO"]
        .iter()
        .any(|marker| locale.split(['-', '_']).any(|part| part == *marker));
    match traditional {
        true => "zh-Hant",
        false => "zh-Hans",
    }
}

fn system_language(locale: &str) -> Option<&'static Language> {
    let normalized = locale.replace('_', "-");
    let normalized = normalized.split(['.', '@']).next().unwrap_or_default();
    match base_language(normalized) {
        "zh" => language_by_code(chinese_variant(normalized)),
        "nb" | "no" => language_by_code("nb-NO"),
        base => language_by_code(normalized).or_else(|| language_by_code(base)),
    }
}

pub fn resolve_language(
    preference: Option<&str>,
    system_locale: Option<&str>,
) -> &'static Language {
    preference
        .and_then(language_by_code)
        .or_else(|| system_locale.and_then(system_language))
        .unwrap_or(&ENGLISH)
}
