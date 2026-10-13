use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::slice;

use libmpv2_sys as sys;

pub(crate) type Handle = NonNull<sys::mpv_handle>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    None,
    Str(String),
    Flag(bool),
    Int(i64),
    Double(f64),
    Array(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    pub(crate) fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Node::Int(v) => Some(*v),
            _ => None,
        }
    }
}

/// # Safety
/// `p` must be null or point to a valid NUL-terminated string.
unsafe fn string_from(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// # Safety
/// `node` must be a valid node filled in by mpv.
unsafe fn node_from_raw(node: &sys::mpv_node) -> Node {
    unsafe {
        match node.format {
            sys::mpv_format_MPV_FORMAT_STRING => Node::Str(string_from(node.u.string)),
            sys::mpv_format_MPV_FORMAT_FLAG => Node::Flag(node.u.flag != 0),
            sys::mpv_format_MPV_FORMAT_INT64 => Node::Int(node.u.int64),
            sys::mpv_format_MPV_FORMAT_DOUBLE => Node::Double(node.u.double_),
            format @ (sys::mpv_format_MPV_FORMAT_NODE_ARRAY
            | sys::mpv_format_MPV_FORMAT_NODE_MAP) => {
                let list = node.u.list;
                let (values, keys) = if list.is_null() {
                    (&[][..], ptr::null_mut())
                } else {
                    let list = &*list;
                    let len = usize::try_from(list.num).unwrap_or(0);
                    let values = if len == 0 || list.values.is_null() {
                        &[][..]
                    } else {
                        slice::from_raw_parts(list.values, len)
                    };
                    (values, list.keys)
                };
                if format == sys::mpv_format_MPV_FORMAT_NODE_ARRAY || keys.is_null() {
                    Node::Array(values.iter().map(|v| node_from_raw(v)).collect())
                } else {
                    let keys = slice::from_raw_parts(keys, values.len());
                    Node::Map(
                        keys.iter()
                            .zip(values)
                            .map(|(k, v)| (string_from(*k), node_from_raw(v)))
                            .collect(),
                    )
                }
            }
            _ => Node::None,
        }
    }
}

pub(crate) fn error_string(code: c_int) -> String {
    unsafe { string_from(sys::mpv_error_string(code)) }
}

pub(crate) fn command_ret(ctx: Handle, args: &[&str]) -> Result<Node, c_int> {
    let args = args
        .iter()
        .map(|a| CString::new(*a))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| sys::mpv_error_MPV_ERROR_INVALID_PARAMETER)?;
    let mut ptrs: Vec<*const c_char> = args
        .iter()
        .map(|a| a.as_ptr())
        .chain([ptr::null()])
        .collect();
    let mut out = MaybeUninit::<sys::mpv_node>::zeroed();
    let rc = unsafe { sys::mpv_command_ret(ctx.as_ptr(), ptrs.as_mut_ptr(), out.as_mut_ptr()) };
    if rc < 0 {
        return Err(rc);
    }
    let mut out = unsafe { out.assume_init() };
    let node = unsafe { node_from_raw(&out) };
    unsafe { sys::mpv_free_node_contents(&mut out) };
    Ok(node)
}

pub(crate) fn get_node(ctx: Handle, name: &str) -> Result<Node, c_int> {
    let name = CString::new(name).map_err(|_| sys::mpv_error_MPV_ERROR_INVALID_PARAMETER)?;
    let mut out = MaybeUninit::<sys::mpv_node>::zeroed();
    let rc = unsafe {
        sys::mpv_get_property(
            ctx.as_ptr(),
            name.as_ptr(),
            sys::mpv_format_MPV_FORMAT_NODE,
            out.as_mut_ptr().cast::<c_void>(),
        )
    };
    if rc < 0 {
        return Err(rc);
    }
    let mut out = unsafe { out.assume_init() };
    let node = unsafe { node_from_raw(&out) };
    unsafe { sys::mpv_free_node_contents(&mut out) };
    Ok(node)
}

pub(crate) fn wakeup(ctx: Handle) {
    unsafe { sys::mpv_wakeup(ctx.as_ptr()) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PropValue {
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
    StartFile { entry_id: i64 },
    FileLoaded,
    EndFile { entry_id: i64, reason: EndReason },
    Seek,
    PlaybackRestart,
    Idle,
    Property { id: u64, value: PropValue },
    Other,
}

/// Must only be called from one thread at a time for a given handle.
pub(crate) fn wait_event(ctx: Handle, timeout: f64) -> Option<RawEvent> {
    let event = unsafe { sys::mpv_wait_event(ctx.as_ptr(), timeout) };
    if event.is_null() {
        return None;
    }
    let event = unsafe { &*event };
    let raw = match event.event_id {
        sys::mpv_event_id_MPV_EVENT_NONE => return None,
        sys::mpv_event_id_MPV_EVENT_SHUTDOWN => RawEvent::Shutdown,
        sys::mpv_event_id_MPV_EVENT_START_FILE if !event.data.is_null() => {
            let data = unsafe { &*event.data.cast::<sys::mpv_event_start_file>() };
            RawEvent::StartFile {
                entry_id: data.playlist_entry_id,
            }
        }
        sys::mpv_event_id_MPV_EVENT_FILE_LOADED => RawEvent::FileLoaded,
        sys::mpv_event_id_MPV_EVENT_END_FILE if !event.data.is_null() => {
            let data = unsafe { &*event.data.cast::<sys::mpv_event_end_file>() };
            let reason = match data.reason {
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_EOF => EndReason::Eof,
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_STOP => EndReason::Stop,
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_QUIT => EndReason::Quit,
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR => EndReason::Error(data.error),
                sys::mpv_end_file_reason_MPV_END_FILE_REASON_REDIRECT => EndReason::Redirect,
                _ => EndReason::Unknown,
            };
            RawEvent::EndFile {
                entry_id: data.playlist_entry_id,
                reason,
            }
        }
        sys::mpv_event_id_MPV_EVENT_SEEK => RawEvent::Seek,
        sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART => RawEvent::PlaybackRestart,
        sys::mpv_event_id_MPV_EVENT_IDLE => RawEvent::Idle,
        sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
            let prop = unsafe { &*event.data.cast::<sys::mpv_event_property>() };
            let value = if prop.data.is_null() {
                PropValue::Unavailable
            } else {
                match prop.format {
                    sys::mpv_format_MPV_FORMAT_FLAG => {
                        PropValue::Flag(unsafe { *prop.data.cast::<c_int>() } != 0)
                    }
                    sys::mpv_format_MPV_FORMAT_DOUBLE => {
                        PropValue::Double(unsafe { *prop.data.cast::<f64>() })
                    }
                    _ => PropValue::Unavailable,
                }
            };
            RawEvent::Property {
                id: event.reply_userdata,
                value,
            }
        }
        _ => RawEvent::Other,
    };
    Some(raw)
}
