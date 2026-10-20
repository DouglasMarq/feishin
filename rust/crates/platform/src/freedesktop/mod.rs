mod color_scheme;
mod mpris;
mod sleep_inhibitor;

pub use color_scheme::{current_color_scheme, watch_color_scheme};
pub use mpris::MediaControls;
pub use sleep_inhibitor::SleepInhibitor;
