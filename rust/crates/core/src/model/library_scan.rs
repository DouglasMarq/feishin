#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibraryScan {
    pub scanning: bool,
    pub scanned_items: Option<u64>,
}
