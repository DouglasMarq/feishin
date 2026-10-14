use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

mod format;
mod languages;

pub use format::format_message;
pub use languages::{ENGLISH, LANGUAGES, Language, resolve_language};

pub struct Message {
    pub singular: &'static str,
    pub plural: &'static str,
    pub forms: &'static [&'static str],
}

pub struct Catalog {
    pub language: &'static str,
    pub messages: &'static [Message],
}

mod generated {
    include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
}

pub use generated::CATALOGS;

type MessageKey = (&'static str, &'static str);

pub struct Translator {
    language: &'static Language,
    messages: HashMap<MessageKey, &'static [&'static str]>,
}

impl Translator {
    pub fn for_language(language: &'static Language) -> Self {
        let messages = CATALOGS
            .iter()
            .find(|catalog| catalog.language == language.code)
            .map(|catalog| {
                catalog
                    .messages
                    .iter()
                    .map(|message| ((message.singular, message.plural), message.forms))
                    .collect()
            })
            .unwrap_or_default();
        Self { language, messages }
    }

    pub fn translate(&self, text: &'static str) -> &'static str {
        self.messages
            .get(&(text, ""))
            .and_then(|forms| forms.first())
            .copied()
            .unwrap_or(text)
    }

    pub fn translate_plural(
        &self,
        singular: &'static str,
        plural: &'static str,
        count: u64,
    ) -> &'static str {
        let english = match count {
            1 => singular,
            _ => plural,
        };
        self.messages
            .get(&(singular, plural))
            .and_then(|forms| forms.get(self.language.plural_rule.form_index(count)))
            .copied()
            .unwrap_or(english)
    }
}

static ACTIVE: LazyLock<RwLock<Translator>> =
    LazyLock::new(|| RwLock::new(Translator::for_language(&ENGLISH)));

pub fn translate(text: &'static str) -> &'static str {
    ACTIVE
        .read()
        .map(|translator| translator.translate(text))
        .unwrap_or(text)
}

pub fn translate_plural(singular: &'static str, plural: &'static str, count: u64) -> &'static str {
    ACTIVE
        .read()
        .map(|translator| translator.translate_plural(singular, plural, count))
        .unwrap_or(singular)
}

pub fn plural_count(count: impl TryInto<u64>) -> u64 {
    count.try_into().unwrap_or(u64::MAX)
}

pub fn select_language(preference: Option<&str>) -> &'static Language {
    let system_locale = sys_locale::get_locale();
    let language = resolve_language(preference, system_locale.as_deref());
    if let Err(error) = slint::select_bundled_translation(language.code) {
        tracing::warn!(
            "selecting the {} translation failed: {error:?}",
            language.code
        );
    }
    match ACTIVE.write() {
        Ok(mut active) => *active = Translator::for_language(language),
        Err(error) => tracing::warn!("switching the language failed: {error}"),
    }
    language
}

macro_rules! tr {
    ($text:literal $(, $argument:expr)* $(,)?) => {
        $crate::i18n::format_message(
            $crate::i18n::translate($text),
            &[$(&$argument as &dyn std::fmt::Display),*],
            None,
        )
    };
}

macro_rules! tr_plural {
    ($singular:literal, $plural:literal, $count:expr $(, $argument:expr)* $(,)?) => {{
        let count = $crate::i18n::plural_count($count);
        $crate::i18n::format_message(
            $crate::i18n::translate_plural($singular, $plural, count),
            &[$(&$argument as &dyn std::fmt::Display),*],
            Some(count),
        )
    }};
}

#[cfg(test)]
mod tests;
