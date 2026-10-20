use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use feishin_player::{Player, PlayerConfig, PlayerEvent};

const USAGE: &str =
    "usage: play [--null] [--muted] [--start=MS] [--crossfade=MS] [--linger=MS] <url> [next-url]";
const NULL_OUTPUT_FLAG: &str = "--null";
const MUTED_FLAG: &str = "--muted";
const START_OPTION_PREFIX: &str = "--start=";
const CROSSFADE_OPTION_PREFIX: &str = "--crossfade=";
const LINGER_OPTION_PREFIX: &str = "--linger=";

#[derive(Default)]
struct PlayArguments {
    is_null_output: bool,
    is_muted: bool,
    start_ms: u64,
    crossfade_ms: u64,
    linger_ms: u64,
    urls: Vec<String>,
}

fn milliseconds_after(argument: &str, prefix: &str) -> Option<u64> {
    argument
        .strip_prefix(prefix)
        .map(|milliseconds| milliseconds.parse().unwrap_or(0))
}

fn parse_option_or_url(parsed: &mut PlayArguments, argument: String) {
    let start = milliseconds_after(&argument, START_OPTION_PREFIX);
    let crossfade = milliseconds_after(&argument, CROSSFADE_OPTION_PREFIX);
    let linger = milliseconds_after(&argument, LINGER_OPTION_PREFIX);
    match (start, crossfade, linger) {
        (Some(start_ms), _, _) => parsed.start_ms = start_ms,
        (_, Some(crossfade_ms), _) => parsed.crossfade_ms = crossfade_ms,
        (_, _, Some(linger_ms)) => parsed.linger_ms = linger_ms,
        (None, None, None) => parsed.urls.push(argument),
    }
}

fn parse_argument(parsed: &mut PlayArguments, argument: String) {
    match argument.as_str() {
        NULL_OUTPUT_FLAG => parsed.is_null_output = true,
        MUTED_FLAG => parsed.is_muted = true,
        _ => parse_option_or_url(parsed, argument),
    }
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> PlayArguments {
    let mut parsed = PlayArguments::default();
    for argument in arguments {
        parse_argument(&mut parsed, argument);
    }
    parsed
}

fn player_config(arguments: &PlayArguments) -> PlayerConfig {
    PlayerConfig {
        is_muted: arguments.is_muted,
        volume: match arguments.is_muted {
            true => 0,
            false => PlayerConfig::default().volume,
        },
        crossfade: Duration::from_millis(arguments.crossfade_ms),
        ..PlayerConfig::default()
    }
}

fn create_player(
    arguments: &PlayArguments,
    event_sender: mpsc::Sender<PlayerEvent>,
) -> Option<Player> {
    let on_event = Box::new(move |event: PlayerEvent| {
        event_sender.send(event).ok();
    });
    let config = player_config(arguments);
    let player = match arguments.is_null_output {
        true => Player::with_null_output(config, on_event),
        false => Player::new(config, on_event),
    };
    player.inspect_err(|error| eprintln!("{error}")).ok()
}

fn start_playback(player: &Player, arguments: &PlayArguments, url: &str) -> bool {
    player
        .load(1, url, arguments.start_ms)
        .and_then(|()| match arguments.urls.get(1) {
            Some(next_url) => player.preload_next(2, next_url),
            None => Ok(()),
        })
        .inspect_err(|error| eprintln!("{error}"))
        .is_ok()
}

fn print_events_until_idle(event_receiver: mpsc::Receiver<PlayerEvent>, started: Instant) {
    for event in event_receiver {
        println!("{:>8.3}s {event:?}", started.elapsed().as_secs_f64());
        if event == PlayerEvent::Idle {
            break;
        }
    }
}

fn main() -> ExitCode {
    let arguments = parse_arguments(std::env::args().skip(1));
    let Some(url) = arguments.urls.first().cloned() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let (event_sender, event_receiver) = mpsc::channel();
    let Some(player) = create_player(&arguments, event_sender) else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    if !start_playback(&player, &arguments, &url) {
        return ExitCode::FAILURE;
    }
    print_events_until_idle(event_receiver, started);
    std::thread::sleep(Duration::from_millis(arguments.linger_ms));
    ExitCode::SUCCESS
}
