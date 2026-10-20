#![cfg(test)]
#![cfg(all(unix, not(target_os = "macos")))]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use feishin_platform::SleepInhibitor;

const RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);
const QUIET_PERIOD: Duration = Duration::from_millis(300);
const SCREEN_SAVER_COOKIE: u32 = 7;
const POWER_MANAGEMENT_COOKIE: u32 = 9;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Inhibit {
        service: &'static str,
        app_name: String,
    },
    UnInhibit {
        service: &'static str,
        cookie: u32,
    },
}

struct PrivateBus(Child);

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_private_bus() -> Option<(PrivateBus, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take();
    let bus = PrivateBus(child);
    let mut address = String::new();
    BufReader::new(stdout?).read_line(&mut address).ok()?;
    let address = address.trim().to_owned();
    (!address.is_empty()).then_some((bus, address))
}

struct FakeScreenSaver(Sender<Call>);

#[zbus::interface(name = "org.freedesktop.ScreenSaver")]
impl FakeScreenSaver {
    fn inhibit(&self, app_name: String, _reason: String) -> u32 {
        let service = "ScreenSaver";
        let _ = self.0.send(Call::Inhibit { service, app_name });
        SCREEN_SAVER_COOKIE
    }

    fn un_inhibit(&self, cookie: u32) {
        let service = "ScreenSaver";
        let _ = self.0.send(Call::UnInhibit { service, cookie });
    }
}

struct FakePowerManagement(Sender<Call>);

#[zbus::interface(name = "org.freedesktop.PowerManagement.Inhibit")]
impl FakePowerManagement {
    fn inhibit(&self, app_name: String, _reason: String) -> u32 {
        let service = "PowerManagement";
        let _ = self.0.send(Call::Inhibit { service, app_name });
        POWER_MANAGEMENT_COOKIE
    }

    fn un_inhibit(&self, cookie: u32) {
        let service = "PowerManagement";
        let _ = self.0.send(Call::UnInhibit { service, cookie });
    }
}

fn serve_fake_services(address: &str, calls: Sender<Call>) -> zbus::blocking::Connection {
    zbus::blocking::connection::Builder::address(address)
        .unwrap()
        .name("org.freedesktop.ScreenSaver")
        .unwrap()
        .name("org.freedesktop.PowerManagement")
        .unwrap()
        .serve_at(
            "/org/freedesktop/ScreenSaver",
            FakeScreenSaver(calls.clone()),
        )
        .unwrap()
        .serve_at(
            "/org/freedesktop/PowerManagement/Inhibit",
            FakePowerManagement(calls),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn sorted_calls(calls: &Receiver<Call>, count: usize) -> Vec<Call> {
    let mut received: Vec<Call> = (0..count)
        .map(|_| calls.recv_timeout(RECEIVE_TIMEOUT).unwrap())
        .collect();
    received.sort_by_key(|call| format!("{call:?}"));
    received
}

fn inhibit_call(service: &'static str) -> Call {
    Call::Inhibit {
        service,
        app_name: "Feishin".to_owned(),
    }
}

#[test]
fn inhibits_once_while_playing_and_releases_on_stop() {
    let Some((_bus, address)) = spawn_private_bus() else {
        eprintln!("skipping: dbus-daemon is not available");
        return;
    };
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
    let (sender, calls) = mpsc::channel();
    let _services = serve_fake_services(&address, sender);
    let inhibitor = SleepInhibitor::new("Feishin").unwrap();

    inhibitor.set_inhibited(true).unwrap();
    inhibitor.set_inhibited(true).unwrap();
    assert_eq!(
        sorted_calls(&calls, 2),
        vec![inhibit_call("PowerManagement"), inhibit_call("ScreenSaver")]
    );
    assert!(calls.recv_timeout(QUIET_PERIOD).is_err());

    inhibitor.set_inhibited(false).unwrap();
    assert_eq!(
        sorted_calls(&calls, 2),
        vec![
            Call::UnInhibit {
                service: "PowerManagement",
                cookie: POWER_MANAGEMENT_COOKIE
            },
            Call::UnInhibit {
                service: "ScreenSaver",
                cookie: SCREEN_SAVER_COOKIE
            },
        ]
    );
}
