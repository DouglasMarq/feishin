use crate::ffi::Node;

const UNKNOWN_DEVICE_NAME: &str = "Unknown Device";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

impl AudioDevice {
    fn from_mpv_node(entry: &Node) -> Self {
        let description = non_empty_text_field(entry, "description");
        AudioDevice {
            name: non_empty_text_field(entry, "name")
                .or(description)
                .unwrap_or(UNKNOWN_DEVICE_NAME)
                .to_owned(),
            description: description.unwrap_or_default().to_owned(),
        }
    }
}

pub(crate) fn audio_devices_from_mpv_list(list: &Node) -> Vec<AudioDevice> {
    match list {
        Node::Array(entries) => entries.iter().map(AudioDevice::from_mpv_node).collect(),
        _ => Vec::new(),
    }
}

fn non_empty_text_field<'node>(entry: &'node Node, key: &str) -> Option<&'node str> {
    entry
        .get(key)
        .and_then(Node::as_str)
        .filter(|text| !text.is_empty())
}
