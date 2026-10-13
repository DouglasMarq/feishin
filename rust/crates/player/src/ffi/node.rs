use std::ffi::{CStr, c_char};
use std::slice;

use libmpv2_sys as sys;

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
            Node::Map(entries) => entries
                .iter()
                .find(|(entry_key, _)| entry_key == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Node::Str(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Node::Int(number) => Some(*number),
            _ => None,
        }
    }
}

pub(super) unsafe fn string_from_nullable_c_string(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

pub(super) unsafe fn node_from_mpv(raw_node: &sys::mpv_node) -> Node {
    match raw_node.format {
        sys::mpv_format_MPV_FORMAT_STRING => {
            Node::Str(unsafe { string_from_nullable_c_string(raw_node.u.string) })
        }
        sys::mpv_format_MPV_FORMAT_FLAG => Node::Flag(unsafe { raw_node.u.flag } != 0),
        sys::mpv_format_MPV_FORMAT_INT64 => Node::Int(unsafe { raw_node.u.int64 }),
        sys::mpv_format_MPV_FORMAT_DOUBLE => Node::Double(unsafe { raw_node.u.double_ }),
        sys::mpv_format_MPV_FORMAT_NODE_ARRAY => unsafe {
            array_from_mpv(raw_node.u.list.as_ref())
        },
        sys::mpv_format_MPV_FORMAT_NODE_MAP => unsafe { map_from_mpv(raw_node.u.list.as_ref()) },
        _ => Node::None,
    }
}

unsafe fn array_from_mpv(list: Option<&sys::mpv_node_list>) -> Node {
    let values = unsafe { list_values(list) };
    Node::Array(
        values
            .iter()
            .map(|value| unsafe { node_from_mpv(value) })
            .collect(),
    )
}

unsafe fn map_from_mpv(list: Option<&sys::mpv_node_list>) -> Node {
    let Some(keys) = list.map(|list| list.keys).filter(|keys| !keys.is_null()) else {
        return unsafe { array_from_mpv(list) };
    };
    let values = unsafe { list_values(list) };
    let keys = unsafe { slice::from_raw_parts(keys, values.len()) };
    Node::Map(
        keys.iter()
            .zip(values)
            .map(|(key, value)| unsafe {
                (string_from_nullable_c_string(*key), node_from_mpv(value))
            })
            .collect(),
    )
}

unsafe fn list_values(list: Option<&sys::mpv_node_list>) -> &[sys::mpv_node] {
    let Some(list) = list else {
        return &[];
    };
    let length = usize::try_from(list.num).unwrap_or(0);
    if length == 0 || list.values.is_null() {
        return &[];
    }
    unsafe { slice::from_raw_parts(list.values, length) }
}
