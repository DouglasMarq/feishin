use feishin_core::queue::Uid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaSource<'url> {
    pub uid: Uid,
    pub url: &'url str,
    pub duration_ms: Option<u64>,
}

impl<'url> MediaSource<'url> {
    pub fn new(uid: Uid, url: &'url str) -> Self {
        Self {
            uid,
            url,
            duration_ms: None,
        }
    }

    pub fn with_duration_ms(self, duration_ms: u64) -> Self {
        Self {
            duration_ms: Some(duration_ms),
            ..self
        }
    }
}
