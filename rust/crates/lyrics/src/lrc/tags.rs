const METADATA_TAGS: &[&str] = &[
    "#", "al", "ar", "au", "by", "id", "la", "lang", "length", "lr", "offset", "re", "tool", "ti",
    "ve",
];
const MILLIS_DIGITS: u32 = 3;

pub(super) enum Tag<'line> {
    Time(u64),
    Karaoke(u64),
    Metadata { key: &'line str, value: &'line str },
}

pub(super) fn leading_tags(mut line: &str) -> (Vec<Tag<'_>>, &str) {
    let mut tags = Vec::new();
    while let Some((content, after_tag)) = line
        .strip_prefix('[')
        .and_then(|inner| inner.split_once(']'))
    {
        let Some(tag) = parse_tag(content) else {
            break;
        };
        tags.push(tag);
        line = after_tag;
    }
    (tags, line)
}

fn parse_tag(content: &str) -> Option<Tag<'_>> {
    if let Some(time_ms) = parse_timestamp(content) {
        return Some(Tag::Time(time_ms));
    }
    if let Some((start, duration)) = content.split_once(',')
        && is_digits(start)
        && (duration.is_empty() || is_digits(duration))
    {
        return start.parse().ok().map(Tag::Karaoke);
    }
    parse_metadata_tag(content)
}

fn parse_metadata_tag(content: &str) -> Option<Tag<'_>> {
    let (key, value) = content.split_once(':')?;
    let key = key.trim();
    METADATA_TAGS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(key))
        .then_some(Tag::Metadata { key, value })
}

pub(super) fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

pub(super) fn parse_timestamp(content: &str) -> Option<u64> {
    let (minutes, rest) = content.trim().split_once(':')?;
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (rest, None),
    };
    if !is_digits(minutes) || !is_digits(seconds) || seconds.len() > 2 {
        return None;
    }
    let minutes: u64 = minutes.parse().ok()?;
    let seconds: u64 = seconds.parse().ok()?;
    let millis = fraction_to_millis(fraction)?;
    minutes
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(1000)?
        .checked_add(millis)
}

fn fraction_to_millis(fraction: Option<&str>) -> Option<u64> {
    let Some(fraction) = fraction else {
        return Some(0);
    };
    if !is_digits(fraction) || fraction.len() > 3 {
        return None;
    }
    let value: u64 = fraction.parse().ok()?;
    let missing_digits = MILLIS_DIGITS.checked_sub(u32::try_from(fraction.len()).ok()?)?;
    value.checked_mul(10u64.pow(missing_digits))
}
