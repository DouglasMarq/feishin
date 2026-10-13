mod app;
mod audio;
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
mod ui;

#[cfg(test)]
mod tests;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

const USAGE: &str = "Usage: feishin [--demo]\n\n  --demo  run with a generated library and silent audio, without a server\n";
const UNKNOWN_ARGUMENT_EXIT_CODE: u8 = 2;

enum Invocation {
    Run(app::Mode),
    Help,
    UnknownArgument(String),
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Invocation {
    let mut mode = app::Mode::Real;
    for argument in arguments {
        match argument.as_str() {
            "--demo" => mode = app::Mode::Demo,
            "-h" | "--help" => return Invocation::Help,
            _ => return Invocation::UnknownArgument(argument),
        }
    }
    Invocation::Run(mode)
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
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Invocation::UnknownArgument(argument) => {
            eprint!("unknown argument: {argument}\n\n{USAGE}");
            ExitCode::from(UNKNOWN_ARGUMENT_EXIT_CODE)
        }
        Invocation::Run(mode) => run_app(mode),
    }
}
