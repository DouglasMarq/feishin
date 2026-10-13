use super::tags::{is_digits, parse_timestamp};

pub(super) fn strip_word_times(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after_open)) = rest.split_once('<') {
        output.push_str(before);
        match after_open.split_once('>') {
            Some((cue, after_close)) if parse_timestamp(cue).is_some() => rest = after_close,
            _ => {
                output.push('<');
                rest = after_open;
            }
        }
    }
    output.push_str(rest);
    output
}

pub(super) fn clean_karaoke_text(text: &str) -> String {
    remove_space_before_punctuation(&strip_karaoke_cues(text))
}

fn strip_karaoke_cues(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after_open)) = rest.split_once('(') {
        output.push_str(before);
        match after_open.split_once(')') {
            Some((cue, after_close)) if is_karaoke_cue(cue) => rest = after_close,
            _ => {
                output.push('(');
                rest = after_open;
            }
        }
    }
    output.push_str(rest);
    output
}

fn is_karaoke_cue(cue: &str) -> bool {
    cue.split_once(',')
        .is_some_and(|(start, duration)| is_digits(start) && is_digits(duration))
}

fn remove_space_before_punctuation(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let is_space_before_punctuation =
            character.is_whitespace() && matches!(characters.peek(), Some(',' | '.'));
        if !is_space_before_punctuation {
            cleaned.push(character);
        }
    }
    cleaned
}
