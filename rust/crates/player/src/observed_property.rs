use libmpv2::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservedProperty {
    Pause,
    TimePosition,
    Duration,
    IdleActive,
    StreamTitle,
}

impl ObservedProperty {
    pub(crate) const ALL: [ObservedProperty; 5] = [
        ObservedProperty::Pause,
        ObservedProperty::TimePosition,
        ObservedProperty::Duration,
        ObservedProperty::IdleActive,
        ObservedProperty::StreamTitle,
    ];

    pub(crate) fn reply_id(self) -> u64 {
        match self {
            ObservedProperty::Pause => 1,
            ObservedProperty::TimePosition => 2,
            ObservedProperty::Duration => 3,
            ObservedProperty::IdleActive => 4,
            ObservedProperty::StreamTitle => 5,
        }
    }

    pub(crate) fn from_reply_id(reply_id: u64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|property| property.reply_id() == reply_id)
    }

    pub(crate) fn mpv_name(self) -> &'static str {
        match self {
            ObservedProperty::Pause => "pause",
            ObservedProperty::TimePosition => "time-pos",
            ObservedProperty::Duration => "duration",
            ObservedProperty::IdleActive => "idle-active",
            ObservedProperty::StreamTitle => "metadata/by-key/icy-title",
        }
    }

    pub(crate) fn format(self) -> Format {
        match self {
            ObservedProperty::Pause | ObservedProperty::IdleActive => Format::Flag,
            ObservedProperty::TimePosition | ObservedProperty::Duration => Format::Double,
            ObservedProperty::StreamTitle => Format::String,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_ids_round_trip() {
        for property in ObservedProperty::ALL {
            assert_eq!(
                ObservedProperty::from_reply_id(property.reply_id()),
                Some(property)
            );
        }
        assert_eq!(ObservedProperty::from_reply_id(0), None);
    }
}
