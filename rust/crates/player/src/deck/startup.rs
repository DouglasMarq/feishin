use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use libmpv2::Mpv;

use super::{AUTOMATIC_AUDIO_DEVICE, MAX_VOLUME, Player};
use crate::error::{PlayerError, Result, error_from_mpv};
use crate::event_loop::{self, EventCallback};
use crate::observed_property::ObservedProperty;
use crate::shared::{PlaylistEntries, Shared};
use crate::{PlayerConfig, PlayerEvent};

const STREAM_LAVF_OPTIONS_WITHOUT_RECONNECT_AT_EOF: &str =
    "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5";
const NULL_AUDIO_OUTPUT: &str = "null";
const BEST_EFFORT_DISABLED_OPTIONS: [&str; 3] =
    ["osc", "input-default-bindings", "input-vo-keyboard"];
const EVENT_THREAD_NAME: &str = "mpv-events";

type MpvOption<'value> = (&'static str, &'value str);

impl Player {
    pub fn new(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, None, on_event)
    }

    #[doc(hidden)]
    pub fn with_null_output(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, Some(NULL_AUDIO_OUTPUT), on_event)
    }

    fn create(
        config: PlayerConfig,
        audio_output: Option<&str>,
        on_event: EventCallback,
    ) -> Result<Self> {
        let volume = config.volume.min(MAX_VOLUME).to_string();
        let options = initial_options(&config, &volume, audio_output);
        let mpv = initialize_mpv(&options)?;
        observe_properties(&mpv)?;
        let shared = Arc::new(Shared {
            mpv,
            stop_requested: AtomicBool::new(false),
            entries: Mutex::new(PlaylistEntries::default()),
        });
        let event_thread = spawn_event_thread(Arc::clone(&shared), on_event)?;
        Ok(Self {
            shared,
            event_thread: Some(event_thread),
        })
    }
}

fn initial_options<'value>(
    config: &'value PlayerConfig,
    volume: &'value str,
    audio_output: Option<&'value str>,
) -> Vec<MpvOption<'value>> {
    let mut options = vec![
        ("vo", "null"),
        ("vid", "no"),
        ("audio-display", "no"),
        ("idle", "yes"),
        ("load-scripts", "no"),
        ("ytdl", "no"),
        ("gapless-audio", gapless_audio_value(config.gapless)),
        ("prefetch-playlist", mpv_flag(config.gapless)),
        ("network-timeout", "10"),
        (
            "stream-lavf-o",
            STREAM_LAVF_OPTIONS_WITHOUT_RECONNECT_AT_EOF,
        ),
        ("replaygain", config.replay_gain.mpv_value()),
        ("replaygain-clip", "no"),
        ("volume", volume),
        (
            "audio-device",
            config
                .audio_device
                .as_deref()
                .unwrap_or(AUTOMATIC_AUDIO_DEVICE),
        ),
    ];
    options.extend(audio_output.map(|output| ("ao", output)));
    options
}

fn mpv_flag(enabled: bool) -> &'static str {
    match enabled {
        true => "yes",
        false => "no",
    }
}

fn gapless_audio_value(gapless: bool) -> &'static str {
    match gapless {
        true => "weak",
        false => "no",
    }
}

fn use_c_numeric_locale_required_by_libmpv() {
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
}

fn initialize_mpv(options: &[MpvOption<'_>]) -> Result<Mpv> {
    use_c_numeric_locale_required_by_libmpv();
    Mpv::with_initializer(|initializer| {
        for (name, value) in options {
            initializer.set_property(name, *value)?;
        }
        for name in BEST_EFFORT_DISABLED_OPTIONS {
            initializer.set_property(name, "no").ok();
        }
        Ok(())
    })
    .map_err(error_from_mpv)
}

fn observe_properties(mpv: &Mpv) -> Result<()> {
    ObservedProperty::ALL.into_iter().try_for_each(|property| {
        mpv.observe_property(property.mpv_name(), property.format(), property.reply_id())
            .map_err(error_from_mpv)
    })
}

fn spawn_event_thread(shared: Arc<Shared>, on_event: EventCallback) -> Result<JoinHandle<()>> {
    thread::Builder::new()
        .name(EVENT_THREAD_NAME.into())
        .spawn(move || event_loop::run(shared, on_event))
        .map_err(|error| PlayerError::Mpv(format!("failed to spawn event thread: {error}")))
}
