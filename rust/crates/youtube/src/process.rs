use std::io;
use std::path::Path;
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::process::Command;

use crate::YouTubeError;

const MAX_FAILURE_MESSAGE_CHARS: usize = 200;

pub(crate) async fn run(
    program: &Path,
    arguments: &[String],
    timeout: Duration,
) -> Result<String, YouTubeError> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_elapsed| YouTubeError::TimedOut(timeout.as_secs()))?
        .map_err(launch_error)?;
    successful_stdout(output)
}

fn launch_error(error: io::Error) -> YouTubeError {
    match error.kind() {
        io::ErrorKind::NotFound => YouTubeError::NotInstalled,
        _ => YouTubeError::Launch(error.to_string()),
    }
}

fn successful_stdout(output: Output) -> Result<String, YouTubeError> {
    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        false => Err(YouTubeError::Failed(failure_message(&output))),
    }
}

fn failure_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let last_line = stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("ERROR:").trim());
    match last_line {
        Some(line) => line.chars().take(MAX_FAILURE_MESSAGE_CHARS).collect(),
        None => format!("exited with {}", output.status),
    }
}
