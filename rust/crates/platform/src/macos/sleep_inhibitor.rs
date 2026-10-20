use std::sync::{Mutex, PoisonError};

use keepawake::KeepAwake;

use crate::{PlatformError, Result};

const INHIBIT_REASON: &str = "Playing audio";
const REVERSE_DOMAIN: &str = "org.jeffvli.feishin";

pub struct SleepInhibitor {
    app_name: String,
    assertion: Mutex<Option<KeepAwake>>,
}

impl SleepInhibitor {
    pub fn new(app_name: &str) -> Result<Self> {
        Ok(Self {
            app_name: app_name.to_owned(),
            assertion: Mutex::new(None),
        })
    }

    pub fn set_inhibited(&self, inhibited: bool) -> Result<()> {
        let mut assertion = self
            .assertion
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match (inhibited, assertion.is_some()) {
            (true, false) => *assertion = Some(self.create_assertion()?),
            (false, true) => *assertion = None,
            _ => {}
        }
        Ok(())
    }

    fn create_assertion(&self) -> Result<KeepAwake> {
        keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason(INHIBIT_REASON)
            .app_name(self.app_name.as_str())
            .app_reverse_domain(REVERSE_DOMAIN)
            .create()
            .map_err(inhibitor_error)
    }
}

fn inhibitor_error(error: keepawake::Error) -> PlatformError {
    PlatformError::SleepInhibitor(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inhibiting_twice_keeps_one_assertion_and_releasing_drops_it() {
        let inhibitor = SleepInhibitor::new("feishin-test").unwrap();
        inhibitor.set_inhibited(true).unwrap();
        inhibitor.set_inhibited(true).unwrap();
        assert!(inhibitor.assertion.lock().unwrap().is_some());
        inhibitor.set_inhibited(false).unwrap();
        assert!(inhibitor.assertion.lock().unwrap().is_none());
    }
}
