mod event;
mod node;

use std::ffi::{CString, c_char, c_int, c_void};
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};

use libmpv2_sys as sys;

use crate::error::{PlayerError, Result};

pub(crate) use event::{EndReason, PropertyValue, RawEvent, wait_event_from_event_thread};
pub(crate) use node::Node;
use node::{node_from_mpv, string_from_nullable_c_string};

pub(crate) type Handle = NonNull<sys::mpv_handle>;

pub(crate) fn error_string(code: c_int) -> String {
    unsafe { string_from_nullable_c_string(sys::mpv_error_string(code)) }
}

pub(crate) fn command_with_reply(handle: Handle, arguments: &[&str]) -> Result<Node> {
    let arguments = arguments
        .iter()
        .map(|argument| c_string(argument))
        .collect::<Result<Vec<_>>>()?;
    let mut argument_pointers: Vec<*const c_char> = arguments
        .iter()
        .map(|argument| argument.as_ptr())
        .chain([ptr::null()])
        .collect();
    node_written_by_mpv(|reply| unsafe {
        sys::mpv_command_ret(handle.as_ptr(), argument_pointers.as_mut_ptr(), reply)
    })
}

pub(crate) fn get_property_node(handle: Handle, name: &str) -> Result<Node> {
    let name = c_string(name)?;
    node_written_by_mpv(|reply| unsafe {
        sys::mpv_get_property(
            handle.as_ptr(),
            name.as_ptr(),
            sys::mpv_format_MPV_FORMAT_NODE,
            reply.cast::<c_void>(),
        )
    })
}

pub(crate) fn set_property_in_background(handle: Handle, name: &str, value: &str) -> Result<()> {
    let name = c_string(name)?;
    let value = c_string(value)?;
    let mut value_pointer = value.as_ptr();
    let status = unsafe {
        sys::mpv_set_property_async(
            handle.as_ptr(),
            0,
            name.as_ptr(),
            sys::mpv_format_MPV_FORMAT_STRING,
            (&raw mut value_pointer).cast::<c_void>(),
        )
    };
    match status {
        error if error < 0 => Err(PlayerError::Mpv(error)),
        _ => Ok(()),
    }
}

pub(crate) fn wake_up_event_thread(handle: Handle) {
    unsafe { sys::mpv_wakeup(handle.as_ptr()) }
}

fn c_string(text: &str) -> Result<CString> {
    Ok(CString::new(text)?)
}

fn node_written_by_mpv(write_node: impl FnOnce(*mut sys::mpv_node) -> c_int) -> Result<Node> {
    let mut raw_node = MaybeUninit::<sys::mpv_node>::zeroed();
    let status = write_node(raw_node.as_mut_ptr());
    if status < 0 {
        return Err(PlayerError::Mpv(status));
    }
    let mut raw_node = unsafe { raw_node.assume_init() };
    let node = unsafe { node_from_mpv(&raw_node) };
    unsafe { sys::mpv_free_node_contents(&mut raw_node) };
    Ok(node)
}
