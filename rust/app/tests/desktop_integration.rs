#![cfg(test)]
#![cfg(all(unix, not(target_os = "macos")))]

use std::path::{Path, PathBuf};
use std::process::Command;

const APP_ID: &str = "org.jeffvli.feishin.native";
const ICON_SIZES: [u32; 10] = [16, 24, 32, 48, 64, 72, 96, 128, 256, 512];

fn installer_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/install-desktop-integration.sh")
}

fn run_installer(arguments: &[&str]) -> std::process::Output {
    Command::new("bash")
        .arg(installer_script())
        .args(arguments)
        .output()
        .unwrap()
}

fn desktop_entry_path(prefix: &Path) -> PathBuf {
    prefix
        .join("share/applications")
        .join(format!("{APP_ID}.desktop"))
}

fn installed_paths(prefix: &Path) -> Vec<PathBuf> {
    let icons = ICON_SIZES.iter().map(|size| {
        prefix
            .join(format!("share/icons/hicolor/{size}x{size}/apps"))
            .join(format!("{APP_ID}.png"))
    });
    [
        desktop_entry_path(prefix),
        prefix
            .join("share/metainfo")
            .join(format!("{APP_ID}.metainfo.xml")),
    ]
    .into_iter()
    .chain(icons)
    .collect()
}

fn installed_exec_line(prefix: &Path, executable: Option<&str>) -> String {
    let prefix_text = prefix.to_str().unwrap();
    let arguments: Vec<&str> = std::iter::once(prefix_text).chain(executable).collect();
    let output = run_installer(&arguments);
    assert!(output.status.success(), "{output:?}");
    std::fs::read_to_string(desktop_entry_path(prefix))
        .unwrap()
        .lines()
        .find(|line| line.starts_with("Exec="))
        .unwrap()
        .to_owned()
}

#[test]
fn default_exec_line_runs_feishin_from_the_path() {
    let prefix = tempfile::tempdir().unwrap();
    assert_eq!(installed_exec_line(prefix.path(), None), "Exec=feishin");
}

#[test]
fn plain_executable_paths_are_written_unquoted() {
    let prefix = tempfile::tempdir().unwrap();
    let exec_line = installed_exec_line(prefix.path(), Some("/home/user/.local/bin/feishin"));
    assert_eq!(exec_line, "Exec=/home/user/.local/bin/feishin");
}

#[test]
fn executable_paths_with_reserved_characters_are_quoted_and_escaped() {
    let prefix = tempfile::tempdir().unwrap();
    let executable = r#"/home/my music&co/a"b`c$d\e 100%/feishin"#;
    let exec_line = installed_exec_line(prefix.path(), Some(executable));
    assert_eq!(
        exec_line,
        r#"Exec="/home/my music&co/a\\"b\\`c\\$d\\\\e 100%%/feishin""#
    );
}

#[test]
fn executable_paths_with_a_newline_are_rejected() {
    let prefix = tempfile::tempdir().unwrap();
    let prefix_text = prefix.path().to_str().unwrap();
    let output = run_installer(&[prefix_text, "/bin/fei\nshin"]);
    assert!(!output.status.success());
}

#[test]
fn uninstall_removes_every_installed_file() {
    let prefix = tempfile::tempdir().unwrap();
    let prefix_text = prefix.path().to_str().unwrap();
    assert!(run_installer(&[prefix_text]).status.success());
    assert!(
        installed_paths(prefix.path())
            .iter()
            .all(|path| path.is_file())
    );

    assert!(
        run_installer(&["--uninstall", prefix_text])
            .status
            .success()
    );
    assert!(
        installed_paths(prefix.path())
            .iter()
            .all(|path| !path.exists())
    );
}
