use std::ffi::c_int;

use libmpv2_sys as sys;

use super::Handle;
use crate::shared::PlaylistEntryId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PropertyValue {
    Flag(bool),
    Double(f64),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndReason {
    Eof,
    Stop,
    Quit,
    Error(c_int),
    Redirect,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RawEvent {
    Shutdown,
    StartFile {
        entry_id: PlaylistEntryId,
    },
    FileLoaded,
    EndFile {
        entry_id: PlaylistEntryId,
        reason: EndReason,
    },
    Seek,
    PlaybackRestart,
    Idle,
    Property {
        reply_id: u64,
        value: PropertyValue,
    },
    Other,
}

pub(crate) fn wait_event_from_event_thread(
    handle: Handle,
    timeout_seconds: f64,
) -> Option<RawEvent> {
    let event = unsafe { sys::mpv_wait_event(handle.as_ptr(), timeout_seconds).as_ref() }?;
    match event.event_id {
        sys::mpv_event_id_MPV_EVENT_NONE => None,
        _ => Some(unsafe { raw_event_from(event) }),
    }
}

unsafe fn raw_event_from(event: &sys::mpv_event) -> RawEvent {
    match event.event_id {
        sys::mpv_event_id_MPV_EVENT_SHUTDOWN => RawEvent::Shutdown,
        sys::mpv_event_id_MPV_EVENT_START_FILE => {
            unsafe { event_data::<sys::mpv_event_start_file>(event) }.map_or(
                RawEvent::Other,
                |data| RawEvent::StartFile {
                    entry_id: data.playlist_entry_id,
                },
            )
        }
        sys::mpv_event_id_MPV_EVENT_FILE_LOADED => RawEvent::FileLoaded,
        sys::mpv_event_id_MPV_EVENT_END_FILE => {
            unsafe { event_data::<sys::mpv_event_end_file>(event) }
                .map_or(RawEvent::Other, end_file_event)
        }
        sys::mpv_event_id_MPV_EVENT_SEEK => RawEvent::Seek,
        sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART => RawEvent::PlaybackRestart,
        sys::mpv_event_id_MPV_EVENT_IDLE => RawEvent::Idle,
        sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE => {
            unsafe { event_data::<sys::mpv_event_property>(event) }.map_or(
                RawEvent::Other,
                |property| RawEvent::Property {
                    reply_id: event.reply_userdata,
                    value: unsafe { property_value_from(property) },
                },
            )
        }
        _ => RawEvent::Other,
    }
}

unsafe fn event_data<Data>(event: &sys::mpv_event) -> Option<&Data> {
    unsafe { event.data.cast::<Data>().as_ref() }
}

fn end_file_event(data: &sys::mpv_event_end_file) -> RawEvent {
    RawEvent::EndFile {
        entry_id: data.playlist_entry_id,
        reason: end_reason_from(data),
    }
}

fn end_reason_from(data: &sys::mpv_event_end_file) -> EndReason {
    match data.reason {
        sys::mpv_end_file_reason_MPV_END_FILE_REASON_EOF => EndReason::Eof,
        sys::mpv_end_file_reason_MPV_END_FILE_REASON_STOP => EndReason::Stop,
        sys::mpv_end_file_reason_MPV_END_FILE_REASON_QUIT => EndReason::Quit,
        sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR => EndReason::Error(data.error),
        sys::mpv_end_file_reason_MPV_END_FILE_REASON_REDIRECT => EndReason::Redirect,
        _ => EndReason::Unknown,
    }
}

unsafe fn property_value_from(property: &sys::mpv_event_property) -> PropertyValue {
    match property.format {
        sys::mpv_format_MPV_FORMAT_FLAG => unsafe { property.data.cast::<c_int>().as_ref() }
            .map_or(PropertyValue::Unavailable, |flag| {
                PropertyValue::Flag(*flag != 0)
            }),
        sys::mpv_format_MPV_FORMAT_DOUBLE => unsafe { property.data.cast::<f64>().as_ref() }
            .map_or(PropertyValue::Unavailable, |value| {
                PropertyValue::Double(*value)
            }),
        _ => PropertyValue::Unavailable,
    }
}
