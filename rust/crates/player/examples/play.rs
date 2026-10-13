use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Instant;

use feishin_player::{Player, PlayerConfig, PlayerEvent};

const USAGE: &str = "usage: play [--null] [--start=MS] <url> [next-url]";
const NULL_OUTPUT_FLAG: &str = "--null";
const START_OPTION_PREFIX: &str = "--start=";

#[derive(Default)]
struct PlayArguments {
    null_output: bool,
    start_ms: u64,
    urls: Vec<String>,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> PlayArguments {
    let mut parsed = PlayArguments::default();
    for argument in arguments {
        if argument == NULL_OUTPUT_FLAG {
            parsed.null_output = true;
            continue;
        }
        match argument.strip_prefix(START_OPTION_PREFIX) {
            Some(start_ms) => parsed.start_ms = start_ms.parse().unwrap_or(0),
            None => parsed.urls.push(argument),
        }
    }
    parsed
}

fn create_player(null_output: bool, event_sender: mpsc::Sender<PlayerEvent>) -> Option<Player> {
    let on_event = Box::new(move |event: PlayerEvent| {
        event_sender.send(event).ok();
    });
    let config = PlayerConfig::default();
    let player = match null_output {
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
    let Some(player) = create_player(arguments.null_output, event_sender) else {
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    if !start_playback(&player, &arguments, &url) {
        return ExitCode::FAILURE;
    }
    print_events_until_idle(event_receiver, started);
    ExitCode::SUCCESS
}
