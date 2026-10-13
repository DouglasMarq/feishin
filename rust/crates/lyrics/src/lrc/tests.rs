use super::*;

fn synced(lines: &[(u64, &str)]) -> Vec<LyricLine> {
    lines
        .iter()
        .map(|(ms, text)| LyricLine {
            time_ms: Some(*ms),
            text: (*text).to_owned(),
        })
        .collect()
}

fn unsynced(lines: &[&str]) -> Vec<LyricLine> {
    lines
        .iter()
        .map(|text| LyricLine {
            time_ms: None,
            text: (*text).to_owned(),
        })
        .collect()
}

#[test]
fn parses_fraction_precisions() {
    let lrc = "[00:01]a\n[00:02.5]b\n[00:03.25]c\n[00:04.125]d\n[01:05:50]e";
    assert_eq!(
        parse_lrc(lrc),
        synced(&[
            (1000, "a"),
            (2500, "b"),
            (3250, "c"),
            (4125, "d"),
            (65500, "e")
        ])
    );
}

#[test]
fn expands_multiple_timestamps_and_sorts() {
    let lrc = "[00:10.00][00:30.00]chorus\n[00:20.00]verse\n[00:05.00]intro";
    assert_eq!(
        parse_lrc(lrc),
        synced(&[
            (5000, "intro"),
            (10000, "chorus"),
            (20000, "verse"),
            (30000, "chorus")
        ])
    );
}

#[test]
fn skips_metadata_and_applies_offset() {
    let lrc = "[ar:Artist]\n[ti:Title]\n[al:Album]\n[by:someone]\n[length: 03:20]\n[offset:+500]\n[00:01.00]one\n[00:00.20]zero";
    assert_eq!(parse_lrc(lrc), synced(&[(0, "zero"), (500, "one")]));
}

#[test]
fn negative_offset_delays_lines() {
    assert_eq!(
        parse_lrc("[offset:-250]\n[00:01.00]x"),
        synced(&[(1250, "x")])
    );
}

#[test]
fn handles_crlf_blank_and_long_minutes() {
    let lrc = "[00:01.00] first \r\n[00:02.00] \r\n\r\n[100:00.00]late\r\n";
    assert_eq!(
        parse_lrc(lrc),
        synced(&[(1000, "first"), (2000, ""), (6_000_000, "late")])
    );
}

#[test]
fn ignores_untimed_lines_in_synced_text() {
    let lrc = "{\"t\":0,\"c\":[{\"tx\":\"credits\"}]}\n[00:01.00]a\nstray line\n[xx:yy]b";
    assert_eq!(parse_lrc(lrc), synced(&[(1000, "a")]));
}

#[test]
fn strips_enhanced_word_timestamps() {
    assert_eq!(
        parse_lrc("[00:01.00]<00:01.00>hello <00:01.50>world <b>"),
        synced(&[(1000, "hello world <b>")])
    );
}

#[test]
fn parses_netease_karaoke_format() {
    let lrc = "[1000,500](1000,200)Hello (1200,300)world ,(1500,100)ok\n[500,400]first";
    assert_eq!(
        parse_lrc(lrc),
        synced(&[(500, "first"), (1000, "Hello world,ok")])
    );
}

#[test]
fn plain_text_becomes_unsynced_lines() {
    let text = "\n[Verse 1]\nLine one\r\n\n[Chorus: Someone]\nLine two\n\n";
    assert_eq!(
        parse_lrc(text),
        unsynced(&["[Verse 1]", "Line one", "", "[Chorus: Someone]", "Line two"])
    );
}

#[test]
fn plain_text_drops_lrc_metadata() {
    assert_eq!(
        parse_lrc("[ar:Artist]\n[ti:Title]\nOnly words"),
        unsynced(&["Only words"])
    );
}

#[test]
fn empty_input_yields_no_lines() {
    assert!(parse_lrc("").is_empty());
    assert!(parse_lrc("  \n\n").is_empty());
}

#[test]
fn rejects_malformed_timestamps() {
    assert_eq!(
        parse_lrc("[00:1234]x\n[00:01.1234]y"),
        unsynced(&["[00:1234]x", "[00:01.1234]y"])
    );
}
