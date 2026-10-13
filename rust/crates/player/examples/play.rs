//! Plays a URL for manual checks.
//!
//! `cargo run -p feishin-player --example play -- [--null] [--start=MS] <url> [next-url]`
//!
//! `--null` uses mpv's null audio output. A second URL is preloaded for a gapless transition.

use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Instant;

use feishin_player::{Player, PlayerConfig, PlayerEvent};

fn main() -> ExitCode {
    let mut null_output = false;
    let mut start_ms = 0;
    let mut urls = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--null" {
            null_output = true;
        } else if let Some(ms) = arg.strip_prefix("--start=") {
            start_ms = ms.parse().unwrap_or(0);
        } else {
            urls.push(arg);
        }
    }
    let Some(url) = urls.first().cloned() else {
        eprintln!("usage: play [--null] [--start=MS] <url> [next-url]");
        return ExitCode::FAILURE;
    };

    let (tx, rx) = mpsc::channel();
    let on_event = Box::new(move |event: PlayerEvent| {
        tx.send(event).ok();
    });
    let config = PlayerConfig::default();
    let player = if null_output {
        Player::with_null_output(config, on_event)
    } else {
        Player::new(config, on_event)
    };
    let player = match player {
        Ok(player) => player,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let started = Instant::now();
    let result = player
        .load(1, &url, start_ms)
        .and_then(|()| match urls.get(1) {
            Some(next) => player.preload_next(2, next),
            None => Ok(()),
        });
    if let Err(err) = result {
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }

    for event in rx {
        println!("{:>8.3}s {event:?}", started.elapsed().as_secs_f64());
        if event == PlayerEvent::Idle {
            break;
        }
    }
    ExitCode::SUCCESS
}
