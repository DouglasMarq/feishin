use feishin_core::queue::Uid;

use crate::PlayerEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeckSlot {
    First,
    Second,
}

impl DeckSlot {
    pub(crate) fn other(self) -> Self {
        match self {
            DeckSlot::First => DeckSlot::Second,
            DeckSlot::Second => DeckSlot::First,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Standby {
    Empty,
    Preloading { uid: Uid, loaded: bool },
    Failed { uid: Uid, message: String },
    FadingOut,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RouteOutcome {
    pub(crate) events: Vec<PlayerEvent>,
    pub(crate) promoted: bool,
}

impl RouteOutcome {
    fn forward(event: PlayerEvent) -> Self {
        Self {
            events: vec![event],
            promoted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Routing {
    lead: DeckSlot,
    standby: Standby,
    paused: bool,
}

pub(crate) fn ends_track(event: &PlayerEvent) -> bool {
    matches!(
        event,
        PlayerEvent::TrackEnded { .. } | PlayerEvent::Error { uid: Some(_), .. }
    )
}

impl Routing {
    pub(crate) fn new() -> Self {
        Self {
            lead: DeckSlot::First,
            standby: Standby::Empty,
            paused: false,
        }
    }

    pub(crate) fn lead(&self) -> DeckSlot {
        self.lead
    }

    pub(crate) fn standby_slot(&self) -> DeckSlot {
        self.lead.other()
    }

    pub(crate) fn is_fading(&self) -> bool {
        self.standby == Standby::FadingOut
    }

    pub(crate) fn has_standby(&self) -> bool {
        self.standby != Standby::Empty
    }

    pub(crate) fn standby_is_ready(&self) -> bool {
        matches!(self.standby, Standby::Preloading { loaded: true, .. })
    }

    pub(crate) fn preload(&mut self, uid: Uid) {
        self.standby = Standby::Preloading { uid, loaded: false };
    }

    pub(crate) fn preload_failed(&mut self, uid: Uid, message: String) {
        self.standby = Standby::Failed { uid, message };
    }

    pub(crate) fn clear_standby(&mut self) {
        self.standby = Standby::Empty;
    }

    pub(crate) fn begin_fade(&mut self) -> Option<PlayerEvent> {
        let Standby::Preloading { uid, loaded: true } = self.standby else {
            return None;
        };
        self.lead = self.lead.other();
        self.standby = Standby::FadingOut;
        Some(PlayerEvent::Started { uid })
    }

    pub(crate) fn outgoing_deck_went_idle(&self, slot: DeckSlot, event: &PlayerEvent) -> bool {
        slot != self.lead && self.is_fading() && *event == PlayerEvent::Idle
    }

    pub(crate) fn route(&mut self, slot: DeckSlot, event: PlayerEvent) -> RouteOutcome {
        match slot == self.lead {
            true => self.route_lead_event(event),
            false => self.route_standby_event(event),
        }
    }

    fn route_lead_event(&mut self, event: PlayerEvent) -> RouteOutcome {
        match event {
            PlayerEvent::Paused => self.pause_changed(true),
            PlayerEvent::Resumed => self.pause_changed(false),
            ended if ends_track(&ended) => self.lead_track_ended(ended),
            other => RouteOutcome::forward(other),
        }
    }

    fn pause_changed(&mut self, paused: bool) -> RouteOutcome {
        if self.paused == paused {
            return RouteOutcome::default();
        }
        self.paused = paused;
        RouteOutcome::forward(match paused {
            true => PlayerEvent::Paused,
            false => PlayerEvent::Resumed,
        })
    }

    fn lead_track_ended(&mut self, event: PlayerEvent) -> RouteOutcome {
        match std::mem::replace(&mut self.standby, Standby::Empty) {
            Standby::Preloading { uid, loaded } => {
                self.lead = self.lead.other();
                RouteOutcome {
                    events: [Some(event), loaded.then_some(PlayerEvent::Started { uid })]
                        .into_iter()
                        .flatten()
                        .collect(),
                    promoted: true,
                }
            }
            Standby::Failed { uid, message } => RouteOutcome {
                events: vec![
                    event,
                    PlayerEvent::Error {
                        uid: Some(uid),
                        message,
                    },
                ],
                promoted: false,
            },
            other => {
                self.standby = other;
                RouteOutcome::forward(event)
            }
        }
    }

    fn route_standby_event(&mut self, event: PlayerEvent) -> RouteOutcome {
        match (&mut self.standby, event) {
            (Standby::Preloading { uid, loaded }, PlayerEvent::Started { uid: started_uid })
                if *uid == started_uid =>
            {
                *loaded = true;
                RouteOutcome::default()
            }
            (
                Standby::Preloading { uid, .. },
                PlayerEvent::Error {
                    uid: Some(failed_uid),
                    message,
                },
            ) if *uid == failed_uid => {
                self.standby = Standby::Failed {
                    uid: failed_uid,
                    message,
                };
                RouteOutcome::default()
            }
            (Standby::FadingOut, error @ PlayerEvent::Error { .. }) => RouteOutcome::forward(error),
            _ => RouteOutcome::default(),
        }
    }
}
