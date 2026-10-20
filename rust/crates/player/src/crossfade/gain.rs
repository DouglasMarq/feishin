use std::f64::consts::FRAC_PI_2;

const SILENT_DECIBELS: f64 = -96.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Gain(f64);

impl Gain {
    pub(crate) const UNITY: Gain = Gain(1.0);
    pub(crate) const SILENT: Gain = Gain(0.0);

    pub(crate) fn decibels(self) -> f64 {
        match self.0 {
            linear if linear > 0.0 => (20.0 * linear.log10()).clamp(SILENT_DECIBELS, 0.0),
            _ => SILENT_DECIBELS,
        }
    }

    #[cfg(test)]
    pub(crate) fn linear(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FadeGains {
    pub(crate) outgoing: Gain,
    pub(crate) incoming: Gain,
}

pub(crate) fn equal_power_gains(progress: f64) -> FadeGains {
    let angle = progress.clamp(0.0, 1.0) * FRAC_PI_2;
    FadeGains {
        outgoing: Gain(angle.cos()),
        incoming: Gain(angle.sin()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-9
    }

    #[test]
    fn decibels_are_clamped_between_silence_and_unity() {
        assert!(is_close(Gain::UNITY.decibels(), 0.0));
        assert!(is_close(Gain::SILENT.decibels(), SILENT_DECIBELS));
        assert!(is_close(Gain(0.5).decibels(), -6.020_599_913_279_624));
        assert!(is_close(Gain(1e-9).decibels(), SILENT_DECIBELS));
        assert!(is_close(Gain(2.0).decibels(), 0.0));
        assert!(is_close(Gain(f64::NAN).decibels(), SILENT_DECIBELS));
    }

    #[test]
    fn equal_power_gains_start_silent_and_keep_constant_power() {
        let start = equal_power_gains(0.0);
        assert!(is_close(start.outgoing.linear(), 1.0) && is_close(start.incoming.linear(), 0.0));
        let end = equal_power_gains(1.0);
        assert!(is_close(end.outgoing.linear(), 0.0) && is_close(end.incoming.linear(), 1.0));
        for step in 0..=20 {
            let gains = equal_power_gains(f64::from(step) / 20.0);
            let power = gains.outgoing.linear().powi(2) + gains.incoming.linear().powi(2);
            assert!(is_close(power, 1.0), "{gains:?}");
        }
        assert_eq!(equal_power_gains(-1.0), start);
        assert_eq!(equal_power_gains(3.0), end);
    }
}
