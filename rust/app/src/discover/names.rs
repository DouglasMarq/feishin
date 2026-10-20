const FEATURING_WORDS: [&str; 4] = ["feat", "ft", "with", "prod"];
const ABBREVIATED_FEATURING_WORDS: [&str; 1] = ["part."];
const UNBRACKETED_FEATURING: [&str; 4] = [" feat. ", " feat ", " ft. ", " ft "];

pub fn normalized_name(text: &str) -> String {
    let spaced: String = without_featuring(&text.to_lowercase())
        .chars()
        .map(|character| match character.is_alphanumeric() {
            true => character,
            false => ' ',
        })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn search_query(title: &str) -> String {
    without_featuring(&title.to_lowercase()).trim().to_owned()
}

fn without_featuring(lowercase: &str) -> String {
    let mut kept = String::new();
    let mut rest = lowercase;
    while let Some((before, bracketed, after)) = next_bracket(rest) {
        kept.push_str(before);
        if !is_featuring(bracketed) {
            kept.push_str(bracketed);
        }
        rest = after;
    }
    kept.push_str(rest);
    cut_unbracketed_featuring(&kept).to_owned()
}

fn next_bracket(text: &str) -> Option<(&str, &str, &str)> {
    let open = text.find(['(', '['])?;
    let (before, from_bracket) = text.split_at_checked(open)?;
    let closing = match from_bracket.starts_with('(') {
        true => ')',
        false => ']',
    };
    let close = from_bracket.find(closing)?;
    let (bracketed, after) = from_bracket.split_at_checked(close + closing.len_utf8())?;
    Some((before, bracketed, after))
}

fn is_featuring(bracketed: &str) -> bool {
    let inner = bracketed.trim_matches(['(', ')', '[', ']', ' ']);
    let spelled_out = FEATURING_WORDS.iter().any(|word| {
        inner
            .strip_prefix(word)
            .is_some_and(|rest| rest.starts_with(['.', ' ']))
    });
    spelled_out
        || ABBREVIATED_FEATURING_WORDS
            .iter()
            .any(|word| inner.starts_with(word))
}

fn cut_unbracketed_featuring(text: &str) -> &str {
    UNBRACKETED_FEATURING
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .and_then(|position| text.get(..position))
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_ignore_case_punctuation_and_featured_artists() {
        assert_eq!(
            normalized_name("Lucky Static (feat. Juno Marsh and Otis Fernwood)"),
            "lucky static"
        );
        assert_eq!(
            normalized_name("Copper Buttons (part. Dizzy Kettle)"),
            "copper buttons"
        );
        assert_eq!(normalized_name("Hey Ya!  "), "hey ya");
        assert_eq!(normalized_name("Song ft. Someone"), "song");
        assert_eq!(normalized_name("Peão Todo Tatuado"), "peão todo tatuado");
    }

    #[test]
    fn other_brackets_are_kept_because_they_change_the_song() {
        assert_eq!(normalized_name("Pós (Ao Vivo)"), "pós ao vivo");
        assert_eq!(normalized_name("Track [Remix]"), "track remix");
        assert_eq!(normalized_name("Unclosed (bracket"), "unclosed bracket");
        assert_eq!(normalized_name("Suite (Part 2)"), "suite part 2");
        assert_ne!(
            normalized_name("Suite (Part 1)"),
            normalized_name("Suite (Part 2)")
        );
    }

    #[test]
    fn search_queries_drop_featured_artists() {
        assert_eq!(search_query("Lucky Static (feat. Juno)"), "lucky static");
        assert_eq!(search_query("The River"), "the river");
    }
}
