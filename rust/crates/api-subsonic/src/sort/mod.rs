mod albums;
mod artists;
mod key;
mod songs;

use rand::seq::SliceRandom;

pub use albums::sort_albums;
pub use artists::sort_artists;
pub use songs::sort_songs;

pub fn shuffle<T>(items: &mut [T]) {
    items.shuffle(&mut rand::rng());
}

pub fn paginate<T>(items: Vec<T>, start: usize, limit: usize) -> Vec<T> {
    let remaining = items.into_iter().skip(start);
    match limit {
        0 => remaining.collect(),
        page_size => remaining.take(page_size).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginate_slices() {
        assert_eq!(paginate(vec![1, 2, 3, 4], 1, 2), vec![2, 3]);
        assert_eq!(paginate(vec![1, 2, 3, 4], 2, 0), vec![3, 4]);
        assert!(paginate(vec![1, 2], 5, 2).is_empty());
    }
}
