use std::collections::BTreeSet;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn click(&mut self, index: usize, toggle: bool, range: bool) {
        match self.anchor {
            Some(anchor) if range => self.extend_from_anchor(anchor, index, toggle),
            _ if toggle => self.toggle(index),
            _ => self.select_only(index),
        }
    }

    fn extend_from_anchor(&mut self, anchor: usize, index: usize, keep_selected: bool) {
        if !keep_selected {
            self.selected.clear();
        }
        self.selected.extend(anchor.min(index)..=anchor.max(index));
    }

    fn toggle(&mut self, index: usize) {
        if !self.selected.remove(&index) {
            self.selected.insert(index);
        }
        self.anchor = Some(index);
    }

    fn select_only(&mut self, index: usize) {
        self.selected.clear();
        self.selected.insert(index);
        self.anchor = Some(index);
    }

    pub fn select_all(&mut self, len: usize) {
        self.selected = (0..len).collect();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn contains(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn indexes(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    pub fn changed_since<'selection>(
        &'selection self,
        before: &'selection Self,
    ) -> impl Iterator<Item = usize> + 'selection {
        before
            .selected
            .symmetric_difference(&self.selected)
            .copied()
    }

    pub fn targets(&self, index: usize) -> Vec<usize> {
        if !self.contains(index) {
            return vec![index];
        }
        self.indexes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_select_toggle_and_extend() {
        let mut selection = Selection::default();
        selection.click(2, false, false);
        assert_eq!(selection.indexes(), [2]);
        selection.click(5, false, true);
        assert_eq!(selection.indexes(), [2, 3, 4, 5]);
        selection.click(0, false, true);
        assert_eq!(selection.indexes(), [0, 1, 2]);
        selection.click(7, true, false);
        assert_eq!(selection.indexes(), [0, 1, 2, 7]);
        selection.click(1, true, false);
        assert_eq!(selection.indexes(), [0, 2, 7]);
        selection.click(3, true, true);
        assert_eq!(selection.indexes(), [0, 1, 2, 3, 7]);
        selection.click(4, false, false);
        assert_eq!(selection.indexes(), [4]);
        selection.select_all(3);
        assert_eq!(selection.len(), 3);
        selection.clear();
        assert_eq!(selection.len(), 0);
    }

    #[test]
    fn context_targets_follow_the_selection() {
        let mut selection = Selection::default();
        selection.click(1, false, false);
        selection.click(3, true, false);
        assert_eq!(selection.targets(3), [1, 3]);
        assert_eq!(selection.targets(2), [2]);
    }
}
