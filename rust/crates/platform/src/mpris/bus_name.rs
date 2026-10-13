const REPLACEMENT_CHARACTER: char = '_';

pub(super) fn bus_suffix(app_name: &str) -> String {
    let mut suffix: String = app_name.chars().map(bus_name_character).collect();
    if suffix.is_empty() || suffix.starts_with(|character: char| character.is_ascii_digit()) {
        suffix.insert(0, REPLACEMENT_CHARACTER);
    }
    suffix
}

fn bus_name_character(character: char) -> char {
    match character {
        valid if valid.is_ascii_alphanumeric() || valid == '_' || valid == '-' => {
            valid.to_ascii_lowercase()
        }
        _ => REPLACEMENT_CHARACTER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_suffix_is_valid() {
        assert_eq!(bus_suffix("Feishin"), "feishin");
        assert_eq!(bus_suffix("My Player.2"), "my_player_2");
        assert_eq!(bus_suffix("9lives"), "_9lives");
        assert_eq!(bus_suffix(""), "_");
    }
}
