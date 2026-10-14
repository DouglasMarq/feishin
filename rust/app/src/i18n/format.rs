use std::fmt::Display;
use std::iter::Peekable;
use std::str::Chars;

fn placeholder(characters: &mut Peekable<Chars<'_>>) -> Option<String> {
    let mut name = String::new();
    for character in characters.by_ref() {
        match character {
            '}' => return Some(name),
            '{' => return None,
            _ => name.push(character),
        }
    }
    None
}

fn substitute(
    name: &str,
    arguments: &[&dyn Display],
    count: Option<u64>,
    next_argument: &mut usize,
) -> Option<String> {
    match name {
        "n" => count.map(|count| count.to_string()),
        "" => {
            let argument = arguments.get(*next_argument).map(ToString::to_string);
            *next_argument += 1;
            argument
        }
        index => index
            .parse::<usize>()
            .ok()
            .and_then(|index| arguments.get(index).map(ToString::to_string)),
    }
}

pub fn format_message(template: &str, arguments: &[&dyn Display], count: Option<u64>) -> String {
    let mut formatted = String::with_capacity(template.len());
    let mut characters = template.chars().peekable();
    let mut next_argument = 0;
    while let Some(character) = characters.next() {
        match (character, characters.peek()) {
            ('{', Some('{')) | ('}', Some('}')) => {
                characters.next();
                formatted.push(character);
            }
            ('{', _) => {
                let name = placeholder(&mut characters).unwrap_or_default();
                let value = substitute(&name, arguments, count, &mut next_argument);
                formatted.push_str(&value.unwrap_or_default());
            }
            _ => formatted.push(character),
        }
    }
    formatted
}
