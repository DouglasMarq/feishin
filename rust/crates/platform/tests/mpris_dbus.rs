use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use feishin_platform::{MediaControlEvent, MediaControls, NowPlaying, PlaybackStatus};
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::zvariant::{OwnedValue, Value};

const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_IFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const TIMEOUT: Duration = Duration::from_secs(5);

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

async fn get(props: &PropertiesProxy<'_>, iface: &'static str, name: &str) -> OwnedValue {
    props
        .get(InterfaceName::from_static_str_unchecked(iface), name)
        .await
        .unwrap()
}

async fn playback_status(props: &PropertiesProxy<'_>) -> String {
    String::try_from(get(props, PLAYER_IFACE, "PlaybackStatus").await).unwrap()
}

async fn wait_for_status(props: &PropertiesProxy<'_>, expected: &str) {
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    while playback_status(props).await != expected {
        assert!(
            tokio::time::Instant::now() < deadline,
            "PlaybackStatus never became {expected}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[test]
fn mpris_registers_and_dispatches_events() {
    let (bus, app_name) = match spawn_private_bus() {
        Some((bus, address)) => {
            unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
            (Some(bus), "Feishin".to_owned())
        }
        None if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() => {
            (None, format!("FeishinTest{}", std::process::id()))
        }
        None => {
            eprintln!("skipping: no D-Bus session bus available");
            return;
        }
    };
    let bus_name = format!("org.mpris.MediaPlayer2.{}", app_name.to_lowercase());

    let (events_tx, events) = mpsc::channel();
    let controls = MediaControls::new(
        &app_name,
        Box::new(move |event| {
            let _ = events_tx.send(event);
        }),
    );
    let mut controls = match controls {
        Ok(controls) => controls,
        Err(err) if bus.is_none() => {
            eprintln!("skipping: session bus unusable: {err}");
            return;
        }
        Err(err) => panic!("{err}"),
    };

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

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        let props = PropertiesProxy::builder(&connection)
            .destination(bus_name.as_str())
            .unwrap()
            .path(PATH)
            .unwrap()
            .build()
            .await
            .unwrap();

        let identity = String::try_from(get(&props, ROOT_IFACE, "Identity").await).unwrap();
        assert_eq!(identity, app_name);

        wait_for_status(&props, "Paused").await;

        let metadata: HashMap<String, OwnedValue> =
            HashMap::try_from(get(&props, PLAYER_IFACE, "Metadata").await).unwrap();
        let text = |key: &str| match metadata.get(key).map(|v| &**v) {
            Some(Value::Str(s)) => s.to_string(),
            other => panic!("{key}: {other:?}"),
        };
        assert_eq!(text("xesam:title"), "Test Song");
        assert_eq!(text("xesam:album"), "Test Album");
        assert_eq!(text("mpris:artUrl"), "https://example.com/cover.jpg");
        assert!(matches!(
            metadata.get("mpris:trackid").map(|v| &**v),
            Some(Value::ObjectPath(path)) if path.as_str() == "/org/feishin/track/abc_123"
        ));
        assert!(matches!(
            metadata.get("mpris:length").map(|v| &**v),
            Some(Value::I64(180_000_000))
        ));

        let position = i64::try_from(get(&props, PLAYER_IFACE, "Position").await).unwrap();
        assert_eq!(position, 12_000_000);
        let volume = f64::try_from(get(&props, PLAYER_IFACE, "Volume").await).unwrap();
        assert_eq!(volume, 0.8);

        let call = |method: &'static str| {
            let connection = connection.clone();
            let bus_name = bus_name.clone();
            async move {
                connection
                    .call_method(
                        Some(bus_name.as_str()),
                        PATH,
                        Some(PLAYER_IFACE),
                        method,
                        &(),
                    )
                    .await
                    .unwrap();
            }
        };

        call("PlayPause").await;
        assert_eq!(
            events.recv_timeout(TIMEOUT).unwrap(),
            MediaControlEvent::Toggle
        );
        wait_for_status(&props, "Playing").await;

        call("Next").await;
        assert_eq!(
            events.recv_timeout(TIMEOUT).unwrap(),
            MediaControlEvent::Next
        );

        connection
            .call_method(
                Some(bus_name.as_str()),
                PATH,
                Some(PLAYER_IFACE),
                "Seek",
                &(-5_000_000i64),
            )
            .await
            .unwrap();
        assert_eq!(
            events.recv_timeout(TIMEOUT).unwrap(),
            MediaControlEvent::SeekBy { offset_ms: -5_000 }
        );

        props
            .set(
                InterfaceName::from_static_str_unchecked(PLAYER_IFACE),
                "Volume",
                Value::F64(0.456),
            )
            .await
            .unwrap();
        assert_eq!(
            events.recv_timeout(TIMEOUT).unwrap(),
            MediaControlEvent::SetVolume(0.46)
        );
    });

    drop(controls);
    drop(bus);
}
