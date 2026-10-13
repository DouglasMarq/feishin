#![cfg(test)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use feishin_platform::{MediaControlEvent, MediaControls, NowPlaying, PlaybackStatus};
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::zvariant::{OwnedValue, Value};

const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(20);

struct PrivateBus(Child);

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct TestBus {
    private_bus: Option<PrivateBus>,
    app_name: String,
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

fn test_bus() -> Option<TestBus> {
    match spawn_private_bus() {
        Some((bus, address)) => {
            unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
            Some(TestBus {
                private_bus: Some(bus),
                app_name: "Feishin".to_owned(),
            })
        }
        None if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() => Some(TestBus {
            private_bus: None,
            app_name: format!("FeishinTest{}", std::process::id()),
        }),
        None => {
            eprintln!("skipping: no D-Bus session bus available");
            None
        }
    }
}

async fn get(properties: &PropertiesProxy<'_>, interface: &'static str, name: &str) -> OwnedValue {
    properties
        .get(InterfaceName::from_static_str_unchecked(interface), name)
        .await
        .unwrap()
}

async fn playback_status(properties: &PropertiesProxy<'_>) -> String {
    String::try_from(get(properties, PLAYER_INTERFACE, "PlaybackStatus").await).unwrap()
}

async fn wait_for_status(properties: &PropertiesProxy<'_>, expected: &str) {
    let deadline = tokio::time::Instant::now() + RECEIVE_TIMEOUT;
    while playback_status(properties).await != expected {
        assert!(
            tokio::time::Instant::now() < deadline,
            "PlaybackStatus never became {expected}"
        );
        tokio::time::sleep(STATUS_POLL_INTERVAL).await;
    }
}

fn publish_test_track(controls: &mut MediaControls) {
    controls
        .set_metadata(&NowPlaying {
            track_id: "abc-123".to_owned(),
            title: "Test Song".to_owned(),
            artists: vec!["Test Artist".to_owned()],
            album: Some("Test Album".to_owned()),
            duration_ms: Some(180_000),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
        })
        .unwrap();
    controls
        .set_status(PlaybackStatus::Paused {
            position_ms: 12_000,
        })
        .unwrap();
    controls.set_volume(0.8).unwrap();
}

async fn properties_proxy<'proxy>(
    connection: &zbus::Connection,
    bus_name: &'proxy str,
) -> PropertiesProxy<'proxy> {
    PropertiesProxy::builder(connection)
        .destination(bus_name)
        .unwrap()
        .path(OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap()
}

async fn assert_published_metadata(properties: &PropertiesProxy<'_>) {
    let metadata: HashMap<String, OwnedValue> =
        HashMap::try_from(get(properties, PLAYER_INTERFACE, "Metadata").await).unwrap();
    let text = |key: &str| match metadata.get(key).map(|value| &**value) {
        Some(Value::Str(text)) => text.to_string(),
        other => panic!("{key}: {other:?}"),
    };
    assert_eq!(text("xesam:title"), "Test Song");
    assert_eq!(text("xesam:album"), "Test Album");
    assert_eq!(text("mpris:artUrl"), "https://example.com/cover.jpg");
    assert!(matches!(
        metadata.get("mpris:trackid").map(|value| &**value),
        Some(Value::ObjectPath(path)) if path.as_str() == "/org/feishin/track/abc_123"
    ));
    assert!(matches!(
        metadata.get("mpris:length").map(|value| &**value),
        Some(Value::I64(180_000_000))
    ));
}

async fn assert_published_state(properties: &PropertiesProxy<'_>, app_name: &str) {
    let identity = String::try_from(get(properties, ROOT_INTERFACE, "Identity").await).unwrap();
    assert_eq!(identity, app_name);

    wait_for_status(properties, "Paused").await;
    assert_published_metadata(properties).await;

    let position = i64::try_from(get(properties, PLAYER_INTERFACE, "Position").await).unwrap();
    assert_eq!(position, 12_000_000);
    let volume = f64::try_from(get(properties, PLAYER_INTERFACE, "Volume").await).unwrap();
    assert_eq!(volume, 0.8);
}

async fn call_player_method(connection: &zbus::Connection, bus_name: &str, method: &'static str) {
    connection
        .call_method(
            Some(bus_name),
            OBJECT_PATH,
            Some(PLAYER_INTERFACE),
            method,
            &(),
        )
        .await
        .unwrap();
}

async fn seek_by(connection: &zbus::Connection, bus_name: &str, offset_microseconds: i64) {
    connection
        .call_method(
            Some(bus_name),
            OBJECT_PATH,
            Some(PLAYER_INTERFACE),
            "Seek",
            &offset_microseconds,
        )
        .await
        .unwrap();
}

async fn assert_dispatches_events(
    connection: &zbus::Connection,
    bus_name: &str,
    properties: &PropertiesProxy<'_>,
    events: &mpsc::Receiver<MediaControlEvent>,
) {
    let next_event = || events.recv_timeout(RECEIVE_TIMEOUT).unwrap();

    call_player_method(connection, bus_name, "PlayPause").await;
    assert_eq!(next_event(), MediaControlEvent::Toggle);
    wait_for_status(properties, "Playing").await;

    call_player_method(connection, bus_name, "Next").await;
    assert_eq!(next_event(), MediaControlEvent::Next);

    seek_by(connection, bus_name, -5_000_000).await;
    assert_eq!(
        next_event(),
        MediaControlEvent::SeekBy { offset_ms: -5_000 }
    );

    properties
        .set(
            InterfaceName::from_static_str_unchecked(PLAYER_INTERFACE),
            "Volume",
            Value::F64(0.456),
        )
        .await
        .unwrap();
    assert_eq!(next_event(), MediaControlEvent::SetVolume(0.46));
}

#[test]
fn mpris_registers_and_dispatches_events() {
    let Some(TestBus {
        private_bus,
        app_name,
    }) = test_bus()
    else {
        return;
    };
    let bus_name = format!("org.mpris.MediaPlayer2.{}", app_name.to_lowercase());

    let (event_sender, events) = mpsc::channel();
    let controls = MediaControls::new(
        &app_name,
        Box::new(move |event| {
            let _ = event_sender.send(event);
        }),
    );
    let mut controls = match controls {
        Ok(controls) => controls,
        Err(error) if private_bus.is_none() => {
            eprintln!("skipping: session bus unusable: {error}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    publish_test_track(&mut controls);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        let properties = properties_proxy(&connection, &bus_name).await;
        assert_published_state(&properties, &app_name).await;
        assert_dispatches_events(&connection, &bus_name, &properties, &events).await;
    });

    drop(controls);
    drop(private_bus);
}
