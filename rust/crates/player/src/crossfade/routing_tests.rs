use super::routing::{DeckSlot, RouteOutcome, Routing, Standby};
use crate::PlayerEvent;

fn forwarded(events: Vec<PlayerEvent>) -> RouteOutcome {
    RouteOutcome {
        events,
        promoted: false,
    }
}

fn ended(uid: u64) -> PlayerEvent {
    PlayerEvent::TrackEnded { uid }
}

fn error(uid: u64) -> PlayerEvent {
    PlayerEvent::Error {
        uid: Some(uid),
        message: "broken".into(),
    }
}

fn routing_with_loaded_preload(uid: u64) -> Routing {
    let mut routing = Routing::new();
    routing.preload(uid);
    routing.route(DeckSlot::Second, PlayerEvent::Started { uid });
    routing
}

#[test]
fn lead_events_pass_through_while_standby_events_are_hidden() {
    let mut routing = Routing::new();
    for event in [
        PlayerEvent::Started { uid: 1 },
        PlayerEvent::Paused,
        PlayerEvent::Resumed,
        PlayerEvent::Idle,
    ] {
        assert_eq!(
            routing.route(DeckSlot::First, event.clone()),
            forwarded(vec![event.clone()])
        );
        assert_eq!(
            routing.route(DeckSlot::Second, event),
            RouteOutcome::default()
        );
    }
}

#[test]
fn preload_becomes_ready_when_its_own_file_loads() {
    let mut routing = Routing::new();
    routing.preload(2);
    routing.route(DeckSlot::Second, PlayerEvent::Started { uid: 9 });
    assert!(!routing.standby_is_ready());
    routing.route(DeckSlot::First, PlayerEvent::Started { uid: 2 });
    assert!(!routing.standby_is_ready());
    routing.route(DeckSlot::Second, PlayerEvent::Started { uid: 2 });
    assert!(routing.standby_is_ready());
}

#[test]
fn begin_fade_swaps_the_lead_and_announces_the_incoming_track() {
    let mut routing = Routing::new();
    routing.preload(2);
    assert_eq!(routing.begin_fade(), None);
    let mut routing = routing_with_loaded_preload(2);
    assert_eq!(routing.begin_fade(), Some(PlayerEvent::Started { uid: 2 }));
    assert_eq!(routing.lead(), DeckSlot::Second);
    assert_eq!(routing.standby_slot(), DeckSlot::First);
    assert!(routing.is_fading());
    assert_eq!(routing.begin_fade(), None);
}

#[test]
fn outgoing_track_end_is_hidden_but_its_errors_are_reported() {
    let mut routing = routing_with_loaded_preload(2);
    routing.begin_fade();
    assert_eq!(
        routing.route(DeckSlot::First, PlayerEvent::TrackEnded { uid: 1 }),
        RouteOutcome::default()
    );
    assert_eq!(
        routing.route(DeckSlot::First, PlayerEvent::Idle),
        RouteOutcome::default()
    );
    assert_eq!(
        routing.route(DeckSlot::First, error(1)),
        forwarded(vec![error(1)])
    );
}

#[test]
fn lead_end_without_fade_promotes_a_loaded_preload_like_gapless() {
    let mut routing = routing_with_loaded_preload(2);
    let outcome = routing.route(DeckSlot::First, PlayerEvent::TrackEnded { uid: 1 });
    assert_eq!(
        outcome,
        RouteOutcome {
            events: vec![
                PlayerEvent::TrackEnded { uid: 1 },
                PlayerEvent::Started { uid: 2 }
            ],
            promoted: true,
        }
    );
    assert_eq!(routing.lead(), DeckSlot::Second);
    assert_eq!(
        routing.route(DeckSlot::First, PlayerEvent::Idle),
        RouteOutcome::default()
    );
}

#[test]
fn lead_end_promotes_a_preload_that_is_still_loading() {
    let mut routing = Routing::new();
    routing.preload(2);
    let outcome = routing.route(DeckSlot::First, error(1));
    assert_eq!(
        outcome,
        RouteOutcome {
            events: vec![error(1)],
            promoted: true,
        }
    );
    assert_eq!(
        routing.route(DeckSlot::Second, PlayerEvent::Started { uid: 2 }),
        forwarded(vec![PlayerEvent::Started { uid: 2 }])
    );
}

#[test]
fn failed_preload_is_reported_when_the_lead_ends() {
    let mut routing = Routing::new();
    routing.preload(2);
    assert_eq!(
        routing.route(DeckSlot::Second, error(2)),
        RouteOutcome::default()
    );
    assert!(!routing.standby_is_ready());
    assert_eq!(
        routing.route(DeckSlot::First, PlayerEvent::TrackEnded { uid: 1 }),
        forwarded(vec![PlayerEvent::TrackEnded { uid: 1 }, error(2)])
    );
    assert_eq!(routing.lead(), DeckSlot::First);
    assert!(!routing.has_standby());
}

#[test]
fn lead_end_during_a_fade_keeps_the_fade() {
    let mut routing = routing_with_loaded_preload(2);
    routing.begin_fade();
    assert_eq!(
        routing.route(DeckSlot::Second, PlayerEvent::TrackEnded { uid: 2 }),
        forwarded(vec![PlayerEvent::TrackEnded { uid: 2 }])
    );
    assert!(routing.is_fading());
}

#[test]
fn clearing_the_standby_stops_routing_its_events() {
    let mut routing = routing_with_loaded_preload(2);
    routing.clear_standby();
    assert!(!routing.has_standby());
    assert_eq!(
        routing.route(DeckSlot::First, PlayerEvent::TrackEnded { uid: 1 }),
        forwarded(vec![PlayerEvent::TrackEnded { uid: 1 }])
    );
    assert_eq!(routing.lead(), DeckSlot::First);
    assert_eq!(DeckSlot::First.other().other(), DeckSlot::First);
    assert_ne!(Standby::FadingOut, Standby::Empty);
}

#[test]
fn incoming_deck_unpausing_does_not_report_a_resume() {
    let mut routing = routing_with_loaded_preload(2);
    routing.begin_fade();
    assert_eq!(
        routing.route(DeckSlot::Second, PlayerEvent::Resumed),
        RouteOutcome::default()
    );
    assert_eq!(
        routing.route(DeckSlot::Second, PlayerEvent::Paused),
        forwarded(vec![PlayerEvent::Paused])
    );
    assert_eq!(
        routing.route(DeckSlot::Second, PlayerEvent::Paused),
        RouteOutcome::default()
    );
}

#[test]
fn only_the_fading_deck_going_idle_releases_it() {
    let mut routing = routing_with_loaded_preload(2);
    let idle = PlayerEvent::Idle;
    assert!(!routing.outgoing_deck_went_idle(DeckSlot::Second, &idle));
    routing.begin_fade();
    assert!(routing.outgoing_deck_went_idle(DeckSlot::First, &idle));
    assert!(!routing.outgoing_deck_went_idle(DeckSlot::First, &ended(1)));
    assert!(!routing.outgoing_deck_went_idle(DeckSlot::First, &error(1)));
    assert!(!routing.outgoing_deck_went_idle(DeckSlot::Second, &idle));
}
