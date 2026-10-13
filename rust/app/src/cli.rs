use std::fmt;

use crate::app::Mode;
use crate::demo::{DEMO_SONG_COUNT, DemoLibrary, MAX_DEMO_SONG_COUNT};

#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    Run(Mode),
    Help,
    Misuse(UsageError),
}

#[derive(Debug, PartialEq, Eq)]
pub enum UsageError {
    UnknownArgument(String),
    MissingSongCount,
    InvalidSongCount(String),
}

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(argument) => write!(formatter, "unknown argument: {argument}"),
            Self::MissingSongCount => write!(formatter, "--demo-songs needs a song count"),
            Self::InvalidSongCount(value) => write!(
                formatter,
                "invalid song count for --demo-songs: {value} (expected 1 to {MAX_DEMO_SONG_COUNT})"
            ),
        }
    }
}

pub fn usage() -> String {
    format!(
        "Usage: feishin [--demo] [--demo-songs <count>]\n\n  --demo                run with a \
         generated library and silent audio, without a server\n  --demo-songs <count>  number \
         of songs in the generated library (implies --demo, default {DEMO_SONG_COUNT})\n"
    )
}

pub fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Invocation {
    let mut arguments = arguments.into_iter();
    let mut demo_library = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--demo" => {
                demo_library.get_or_insert_default();
            }
            "--demo-songs" => match parse_song_count(arguments.next()) {
                Ok(library) => demo_library = Some(library),
                Err(error) => return Invocation::Misuse(error),
            },
            "-h" | "--help" => return Invocation::Help,
            _ => return Invocation::Misuse(UsageError::UnknownArgument(argument)),
        }
    }
    Invocation::Run(demo_library.map_or(Mode::Real, Mode::Demo))
}

fn parse_song_count(value: Option<String>) -> Result<DemoLibrary, UsageError> {
    let value = value.ok_or(UsageError::MissingSongCount)?;
    value
        .parse::<usize>()
        .ok()
        .filter(|count| (1..=MAX_DEMO_SONG_COUNT).contains(count))
        .map(DemoLibrary::with_songs)
        .ok_or(UsageError::InvalidSongCount(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Invocation {
        parse_arguments(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn no_arguments_run_against_a_real_server() {
        assert_eq!(parse(&[]), Invocation::Run(Mode::Real));
    }

    #[test]
    fn demo_uses_the_default_library() {
        assert_eq!(
            parse(&["--demo"]),
            Invocation::Run(Mode::Demo(DemoLibrary::default()))
        );
    }

    #[test]
    fn demo_songs_sizes_the_library_in_any_order() {
        let expected = Invocation::Run(Mode::Demo(DemoLibrary::with_songs(100_000)));
        assert_eq!(parse(&["--demo", "--demo-songs", "100000"]), expected);
        assert_eq!(parse(&["--demo-songs", "100000", "--demo"]), expected);
        assert_eq!(parse(&["--demo-songs", "100000"]), expected);
    }

    #[test]
    fn bad_song_counts_are_usage_errors() {
        assert_eq!(
            parse(&["--demo", "--demo-songs"]),
            Invocation::Misuse(UsageError::MissingSongCount)
        );
        for value in ["0", "lots", "-5", "99999999999"] {
            assert_eq!(
                parse(&["--demo-songs", value]),
                Invocation::Misuse(UsageError::InvalidSongCount(value.to_owned()))
            );
        }
    }

    #[test]
    fn help_and_unknown_arguments() {
        assert_eq!(parse(&["--demo", "-h"]), Invocation::Help);
        assert_eq!(
            parse(&["--bogus"]),
            Invocation::Misuse(UsageError::UnknownArgument("--bogus".into()))
        );
        assert!(usage().contains("--demo-songs <count>"));
    }
}
