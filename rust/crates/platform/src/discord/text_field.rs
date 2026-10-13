const MAX_FIELD_LENGTH: usize = 127;
const MIN_FIELD_LENGTH: usize = 2;
const ELLIPSIS: char = '\u{2026}';

pub(super) fn text_field(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        return fallback.to_owned();
    }
    let mut padded = value.to_owned();
    for _missing_character in value.chars().count()..MIN_FIELD_LENGTH {
        padded.push(' ');
    }
    truncate_to_max_length(&padded)
}

fn truncate_to_max_length(field: &str) -> String {
    if field.chars().count() <= MAX_FIELD_LENGTH {
        return field.to_owned();
    }
    let mut truncated: String = field.chars().take(MAX_FIELD_LENGTH - 1).collect();
    truncated.push(ELLIPSIS);
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_fields_are_truncated() {
        let long = "x".repeat(200);
        let truncated = text_field(&long, "Idle");
        assert_eq!(truncated.chars().count(), MAX_FIELD_LENGTH);
        assert!(truncated.ends_with(ELLIPSIS));
        assert_eq!(text_field(&"y".repeat(127), "Idle").chars().count(), 127);
    }
}
