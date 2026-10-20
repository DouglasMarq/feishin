use std::cell::RefCell;

use slint::Image;

use crate::ui::cover_lru::CoverLru;

const SHARED_COVER_CAPACITY: usize = 96;

thread_local! {
    static SHARED_COVERS: RefCell<CoverLru<String, Image>> = RefCell::new(CoverLru::new(SHARED_COVER_CAPACITY));
}

pub fn cached_cover(cover_id: &str) -> Option<Image> {
    SHARED_COVERS.with_borrow_mut(|covers| covers.get(cover_id))
}

pub fn remember_cover(cover_id: String, image: Image) {
    SHARED_COVERS.with_borrow_mut(|covers| {
        covers.insert(cover_id, image);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_cache_evicts_least_recently_used() {
        let mut covers: CoverLru<String, Image> = CoverLru::new(2);
        covers.insert("a".into(), Image::default());
        covers.insert("b".into(), Image::default());
        assert!(covers.get("a").is_some());
        covers.insert("c".into(), Image::default());
        assert!(covers.get("b").is_none());
        assert!(covers.get("a").is_some() && covers.get("c").is_some());
    }
}
