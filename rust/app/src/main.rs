mod app;
mod audio;
mod connect;
mod controller;
mod covers;
mod demo;
mod library;
mod media;
mod queue;
mod scrobble;
mod session;
mod store;
mod ui;

#[cfg(test)]
mod tests;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

const USAGE: &str = "Usage: feishin [--demo]\n\n  --demo  run with a generated library and silent audio, without a server\n";

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let mut mode = app::Mode::Real;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--demo" => mode = app::Mode::Demo,
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprint!("unknown argument: {other}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

    match app::run(mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("feishin: {e:#}");
            ExitCode::FAILURE
        }
    }
}
