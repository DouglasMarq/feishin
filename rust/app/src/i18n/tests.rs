use super::languages::{PluralRule, language_by_code};
use super::*;

fn language(code: &str) -> &'static Language {
    language_by_code(code).unwrap()
}

#[test]
fn a_supported_preference_wins_over_the_system_locale() {
    assert_eq!(resolve_language(Some("de"), Some("fr-FR")).code, "de");
}

#[test]
fn an_unknown_preference_falls_back_to_the_system_locale() {
    assert_eq!(resolve_language(Some("xx"), Some("fr-FR")).code, "fr");
    assert_eq!(resolve_language(None, Some("pt_BR.UTF-8")).code, "pt-BR");
    assert_eq!(resolve_language(None, Some("pt-PT")).code, "pt");
}

#[test]
fn chinese_and_norwegian_locales_map_to_their_variants() {
    assert_eq!(resolve_language(None, Some("zh-CN")).code, "zh-Hans");
    assert_eq!(resolve_language(None, Some("zh_TW.UTF-8")).code, "zh-Hant");
    assert_eq!(resolve_language(None, Some("zh-Hant-HK")).code, "zh-Hant");
    assert_eq!(resolve_language(None, Some("nb")).code, "nb-NO");
}

#[test]
fn unsupported_or_missing_locales_use_english() {
    assert_eq!(resolve_language(None, Some("kl-GL")).code, "en");
    assert_eq!(resolve_language(None, None).code, "en");
}

#[test]
fn plural_rules_pick_the_gettext_form() {
    let cases = [
        (PluralRule::OneOther, [(0, 1), (1, 0), (2, 1)].as_slice()),
        (PluralRule::ZeroAndOneSingular, &[(0, 0), (1, 0), (2, 1)]),
        (PluralRule::SingleForm, &[(1, 0), (5, 0)]),
        (PluralRule::Czech, &[(1, 0), (3, 1), (5, 2), (0, 2)]),
        (
            PluralRule::Latvian,
            &[(0, 2), (1, 0), (11, 1), (21, 0), (5, 1)],
        ),
        (
            PluralRule::Polish,
            &[(1, 0), (2, 1), (12, 2), (22, 1), (25, 2)],
        ),
        (
            PluralRule::Romanian,
            &[(1, 0), (0, 1), (19, 1), (20, 2), (101, 1)],
        ),
        (
            PluralRule::EastSlavic,
            &[(1, 0), (11, 2), (21, 0), (3, 1), (13, 2), (5, 2)],
        ),
        (
            PluralRule::Slovenian,
            &[(1, 0), (102, 1), (3, 2), (5, 3), (101, 0)],
        ),
        (
            PluralRule::Arabic,
            &[(0, 0), (1, 1), (2, 2), (5, 3), (11, 4), (100, 5), (103, 3)],
        ),
    ];
    for (rule, expectations) in cases {
        for (count, form) in expectations {
            assert_eq!(rule.form_index(*count), *form, "{rule:?} with {count}");
        }
    }
}

#[test]
fn every_catalog_is_a_known_language_with_complete_plural_forms() {
    for catalog in CATALOGS {
        let language = language_by_code(catalog.language)
            .unwrap_or_else(|| panic!("{} has no language entry", catalog.language));
        for message in catalog
            .messages
            .iter()
            .filter(|message| !message.plural.is_empty())
        {
            assert_eq!(
                message.forms.len(),
                language.plural_rule.form_count(),
                "{}: {}",
                catalog.language,
                message.singular
            );
        }
    }
}

#[test]
fn every_language_except_english_has_a_catalog() {
    for language in LANGUAGES.iter().filter(|language| language.code != "en") {
        assert!(
            CATALOGS
                .iter()
                .any(|catalog| catalog.language == language.code),
            "{} has no catalog",
            language.code
        );
    }
}

#[test]
fn untranslated_messages_fall_back_to_english() {
    let translator = Translator::for_language(language("de"));
    assert_eq!(
        translator.translate("not a message in any catalog"),
        "not a message in any catalog"
    );
    assert_eq!(
        translator.translate_plural("{n} thing", "{n} things", 1),
        "{n} thing"
    );
    assert_eq!(
        translator.translate_plural("{n} thing", "{n} things", 4),
        "{n} things"
    );
}

#[test]
fn english_uses_the_source_strings() {
    let translator = Translator::for_language(&ENGLISH);
    assert_eq!(translator.translate("Albums"), "Albums");
}

#[test]
fn messages_substitute_positional_indexed_and_count_placeholders() {
    let arguments: [&dyn std::fmt::Display; 2] = [&"a", &"b"];
    assert_eq!(format_message("{} and {}", &arguments, None), "a and b");
    assert_eq!(
        format_message("{1} before {0}", &arguments, None),
        "b before a"
    );
    assert_eq!(
        format_message("{n} songs in {}", &arguments, Some(3)),
        "3 songs in a"
    );
    assert_eq!(format_message("{{literal}}", &[], None), "{literal}");
    assert_eq!(format_message("missing {}", &[], None), "missing ");
}

#[test]
fn the_macros_format_untranslated_messages() {
    assert_eq!(tr!("Cannot share: {}", "offline"), "Cannot share: offline");
    assert_eq!(tr_plural!("{n} song", "{n} songs", 2_usize), "2 songs");
    assert_eq!(
        tr_plural!("{n} song in {}", "{n} songs in {}", 1_u32, "Rock"),
        "1 song in Rock"
    );
}
