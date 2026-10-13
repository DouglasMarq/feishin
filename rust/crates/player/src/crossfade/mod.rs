mod gain;
mod routing;
mod schedule;

#[cfg(test)]
mod routing_tests;

pub(crate) use gain::Gain;
pub(crate) use routing::{DeckSlot, Routing, ends_track};
pub(crate) use schedule::{
    FADE_STEP, FadeWindow, FaderPlan, OUTPUT_BUFFER_SECONDS, plan_before_fade, plan_during_fade,
};
