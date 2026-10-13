mod app;
mod audio;
mod cli;
mod connect;
mod controller;
mod covers;
mod demo;
mod discord;
mod library;
mod media;
mod play_queue_sync;
mod scrobble;
mod session;
mod sleep_timer;
mod store;
mod termination;
mod ui;

#[cfg(test)]
mod tests;

use std::process::ExitCode;

use cli::{Invocation, parse_arguments, usage};
use tracing_subscriber::EnvFilter;

const USAGE_ERROR_EXIT_CODE: u8 = 2;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();
}

fn run_app(mode: app::Mode) -> ExitCode {
    match app::run(mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("feishin: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    init_tracing();
    match parse_arguments(std::env::args().skip(1)) {
        Invocation::Help => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        Invocation::Misuse(error) => {
            eprint!("{error}\n\n{}", usage());
            ExitCode::from(USAGE_ERROR_EXIT_CODE)
        }
        Invocation::Run(mode) => run_app(mode),
    }
}
