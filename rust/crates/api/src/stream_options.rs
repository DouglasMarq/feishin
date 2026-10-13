#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamOptions {
    pub max_bitrate_kbps: Option<u32>,
    pub format: Option<String>,
}
