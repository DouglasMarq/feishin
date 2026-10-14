use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use mpris_server::zbus::blocking::{Connection, Proxy};

use crate::{PlatformError, Result};

const THREAD_NAME: &str = "sleep-inhibitor";
const INHIBIT_REASON: &str = "Playing audio";

struct InhibitService {
    destination: &'static str,
    path: &'static str,
    interface: &'static str,
}

const INHIBIT_SERVICES: [InhibitService; 2] = [
    InhibitService {
        destination: "org.freedesktop.ScreenSaver",
        path: "/org/freedesktop/ScreenSaver",
        interface: "org.freedesktop.ScreenSaver",
    },
    InhibitService {
        destination: "org.freedesktop.PowerManagement",
        path: "/org/freedesktop/PowerManagement/Inhibit",
        interface: "org.freedesktop.PowerManagement.Inhibit",
    },
];

struct Inhibition {
    proxy: Proxy<'static>,
    cookie: u32,
}

pub struct SleepInhibitor {
    requests: Sender<bool>,
}

impl SleepInhibitor {
    pub fn new(app_name: &str) -> Result<Self> {
        let connection = Connection::session().map_err(inhibitor_error)?;
        let (requests, receiver) = mpsc::channel();
        let app_name = app_name.to_owned();
        thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || run(&connection, &app_name, &receiver))
            .map_err(inhibitor_error)?;
        Ok(Self { requests })
    }

    pub fn set_inhibited(&self, inhibited: bool) -> Result<()> {
        self.requests
            .send(inhibited)
            .map_err(|_| PlatformError::SleepInhibitor("sleep inhibitor thread is gone".to_owned()))
    }
}

fn inhibitor_error(error: impl ToString) -> PlatformError {
    PlatformError::SleepInhibitor(error.to_string())
}

fn run(connection: &Connection, app_name: &str, requests: &Receiver<bool>) {
    let mut inhibited = false;
    let mut inhibitions = Vec::new();
    for wanted in requests {
        if wanted == inhibited {
            continue;
        }
        inhibited = wanted;
        match wanted {
            true => inhibitions = inhibit_all(connection, app_name),
            false => inhibitions.drain(..).for_each(release),
        }
    }
}

fn inhibit_all(connection: &Connection, app_name: &str) -> Vec<Inhibition> {
    INHIBIT_SERVICES
        .iter()
        .filter_map(|service| {
            inhibit(connection, service, app_name)
                .inspect_err(|error| {
                    tracing::warn!("{} inhibit failed: {error}", service.interface)
                })
                .ok()
        })
        .collect()
}

fn inhibit(
    connection: &Connection,
    service: &InhibitService,
    app_name: &str,
) -> mpris_server::zbus::Result<Inhibition> {
    let proxy = Proxy::new(
        connection,
        service.destination,
        service.path,
        service.interface,
    )?;
    let cookie = proxy.call("Inhibit", &(app_name, INHIBIT_REASON))?;
    Ok(Inhibition { proxy, cookie })
}

fn release(inhibition: Inhibition) {
    if let Err(error) = inhibition
        .proxy
        .call::<_, _, ()>("UnInhibit", &(inhibition.cookie,))
    {
        tracing::warn!("{} release failed: {error}", inhibition.proxy.interface());
    }
}
