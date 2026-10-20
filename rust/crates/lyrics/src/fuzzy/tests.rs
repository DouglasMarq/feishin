use super::*;

fn item<'text>(name: &'text str, artist: &'text str, is_sync: Option<bool>) -> SearchItem<'text> {
    SearchItem {
        name,
        artist,
        is_sync,
    }
}

#[test]
fn exact_match_scores_near_zero() {
    let items = [item("Keqivuow Zqoxbemh", "Yaiiw", None)];
    let ranked = order_search_results("Keqivuow Zqoxbemh", "Yaiiw", &items);
    assert_eq!(ranked.len(), 1);
    assert!(ranked[0].score < 1e-6);
}

#[test]
fn unrelated_items_are_dropped() {
    let items = [item("Levxticith Munniziwc", "Wekemh Swefw", None)];
    assert!(order_search_results("Keqivuow Zqoxbemh", "Yaiiw", &items).is_empty());
}

#[test]
fn empty_query_matches_nothing() {
    let items = [item("Song", "Artist", None)];
    assert!(order_search_results("", "", &items).is_empty());
}

type ReferenceItem<'text> = (&'text str, &'text str, Option<bool>);

fn assert_matches_fuse_js_7_4_2(
    name: &str,
    artist: &str,
    raw_items: &[ReferenceItem<'_>],
    expected: &[(usize, f64)],
) {
    let items: Vec<SearchItem<'_>> = raw_items
        .iter()
        .map(|&(item_name, item_artist, is_sync)| item(item_name, item_artist, is_sync))
        .collect();
    let ranked = order_search_results(name, artist, &items);
    let got: Vec<usize> = ranked.iter().map(|result| result.index).collect();
    let want: Vec<usize> = expected.iter().map(|(index, _)| *index).collect();
    assert_eq!(got, want, "order for {name:?} / {artist:?}");
    for (result, (_, score)) in ranked.iter().zip(expected) {
        let tolerance = score.abs() * 1e-9 + 1e-18;
        assert!(
            (result.score - score).abs() <= tolerance,
            "{name:?}/{artist:?} item {}: got {}, want {score}",
            result.index,
            result.score
        );
    }
}

#[test]
fn reference_scores_for_title_and_artist() {
    assert_matches_fuse_js_7_4_2(
        "Keqivuow Zqoxbemh",
        "Yaiiw",
        &[
            ("Keqivuow Zqoxbemh", "Yaiiw", None),
            ("Keqivuow Zqoxbemh (Zivobcizim 2011)", "Yaiiw", Some(true)),
            ("Keqivouw Zoxbemh", "Yaiiw", Some(false)),
            ("Keqivuow Zqoxbemh", "Xowul! Oc Cqi Muble", Some(true)),
            ("Suttiz Yaiiw", "Yaiiw", None),
            ("Keqivuow Zqoxbemh kh Yaiiw", "Yaiiw", None),
        ],
        &[
            (1, 0.03162277660168379),
            (0, 8.569061098350962e-12),
            (5, 0.03162277660168379),
            (2, 0.29335759711558734),
        ],
    );
}

#[test]
fn reference_scores_for_multi_word_artist() {
    assert_matches_fuse_js_7_4_2(
        "Tivew",
        "Siwbqu Hewija",
        &[
            ("Tivew", "Siwbqu Hewija", None),
            ("Tivew (Tudi)", "Siwbqu Hewija, Paibc", None),
            ("Tivew Czii", "Neetb Pozmiw", None),
            ("Tivewomi", "Siwbqu", None),
        ],
        &[
            (0, 8.569061098350962e-12),
            (1, 0.01857804455091699),
            (3, 0.5384615384615384),
        ],
    );
}

#[test]
fn reference_scores_for_patterns_longer_than_32_characters() {
    assert_matches_fuse_js_7_4_2(
        "A Very Long Song Title That Exceeds Thirty Two Characters",
        "Some Long Artist Name With Many Words In It Too",
        &[
            (
                "A Very Long Song Title That Exceeds Thirty Two Characters",
                "Some Long Artist Name With Many Words In It Too",
                Some(true),
            ),
            (
                "A Very Long Song Title That Exceeds 32 Characters (Live)",
                "Some Long Artist Name With Many Words",
                None,
            ),
            ("Very Long Song", "Some Long Artist", Some(false)),
        ],
        &[
            (0, 0.00001131032569734829),
            (1, 0.556782511481585),
            (2, 0.8672410917975312),
        ],
    );
}

#[test]
fn reference_scores_for_accented_title() {
    assert_matches_fuse_js_7_4_2(
        "Loné mit Voz",
        "Iwizph 52",
        &[
            ("Loné Mit Voz", "Iwizph 52", None),
            ("Loni mit Voz", "Iwizph52", None),
            ("星めおょ", "Iwizph 52", None),
        ],
        &[(0, 9.287439764962262e-10), (1, 0.2384033869402615)],
    );
}

#[test]
fn reference_scores_for_title_only() {
    assert_matches_fuse_js_7_4_2(
        "Hibcizmoh",
        "",
        &[
            ("Hibcizmoh", "Cqi Kioctib", None),
            ("Hibcizmoh (Zivobcizim)", "Cqi Kioctib", Some(true)),
            ("Hibcizmohb", "Pawb W Zebib", None),
            ("Cemoh", "G", None),
        ],
        &[
            (1, 0.08699614330652679),
            (0, 1.4901161193847656e-8),
            (2, 0.03162277660168379),
            (3, 0.7453559924999299),
        ],
    );
}

#[test]
fn reference_scores_for_artist_only() {
    assert_matches_fuse_js_7_4_2(
        "",
        "Zomueqiom",
        &[
            ("Lziix", "Zomueqiom", None),
            ("Sozvo Xetuli", "Zomueqiom, Cqev", None),
            ("Lziix", "Zomue Qiom", None),
            ("Lziix", "CTL", None),
        ],
        &[
            (0, 1.4901161193847656e-8),
            (1, 0.08699614330652679),
            (2, 0.459912562959988),
        ],
    );
}

#[test]
fn reference_scores_for_japanese_text() {
    assert_matches_fuse_js_7_4_2(
        "星めおょむきをむぉぢぺぶまのえそ",
        "森川青木",
        &[
            ("星めおょむきをむぉぢぺぶまのえそ", "森川青木", None),
            ("星めおょ", "森川 青木", None),
        ],
        &[(0, 2.220446049250313e-16)],
    );
}

#[test]
fn reference_scores_skip_blank_names() {
    assert_matches_fuse_js_7_4_2(
        "Awmiz Xzibbazi",
        "Yaiiw",
        &[
            ("Keqivuow Zqoxbemh kh Yaiiw", "Yaiiw", None),
            (
                "Awmiz Xzibbazi kh Yaiiw & Modum Kefui",
                "Yaiiw & Modum Kefui",
                None,
            ),
            ("  ", "Yaiiw", None),
        ],
        &[(1, 0.07345138681571149)],
    );
}
