use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;

use crate::YouTubeError;

const MAX_FAILURE_MESSAGE_CHARS: usize = 200;
#[cfg(target_os = "macos")]
const EXTRA_SEARCH_DIRECTORIES: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];
#[cfg(not(target_os = "macos"))]
const EXTRA_SEARCH_DIRECTORIES: &[&str] = &[];

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
    if let Some(search_path) = search_path_with(std::env::var_os("PATH"), EXTRA_SEARCH_DIRECTORIES)
    {
        command.env("PATH", search_path);
    }
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_elapsed| YouTubeError::TimedOut(timeout))?
        .map_err(launch_error)?;
    successful_stdout(output)
}

fn search_path_with(current: Option<OsString>, extra_directories: &[&str]) -> Option<OsString> {
    let mut directories: Vec<PathBuf> = current
        .as_deref()
        .map(|path| std::env::split_paths(path).collect())
        .unwrap_or_default();
    let missing: Vec<PathBuf> = extra_directories
        .iter()
        .map(PathBuf::from)
        .filter(|directory| !directories.contains(directory))
        .collect();
    if missing.is_empty() {
        return None;
    }
    directories.extend(missing);
    std::env::join_paths(directories).ok()
}

fn launch_error(error: io::Error) -> YouTubeError {
    match error.kind() {
        io::ErrorKind::NotFound => YouTubeError::NotInstalled,
        _ => YouTubeError::Launch(Arc::new(error)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_directories_are_appended_to_the_search_path() {
        let joined = search_path_with(
            Some(OsString::from("/usr/bin:/bin")),
            &["/opt/homebrew/bin"],
        );
        assert_eq!(
            joined,
            Some(OsString::from("/usr/bin:/bin:/opt/homebrew/bin"))
        );
    }

    #[test]
    fn a_complete_search_path_is_left_alone() {
        let current = Some(OsString::from("/opt/homebrew/bin:/usr/bin"));
        assert_eq!(search_path_with(current, &["/opt/homebrew/bin"]), None);
        assert_eq!(
            search_path_with(Some(OsString::from("/usr/bin")), &[]),
            None
        );
    }
}
