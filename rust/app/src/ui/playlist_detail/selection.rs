use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode {
    Select,
    Toggle,
    SelectRange,
    AddRange,
}

impl ClickMode {
    pub fn from_modifiers(toggle_held: bool, range_held: bool) -> Self {
        match (toggle_held, range_held) {
            (false, false) => Self::Select,
            (true, false) => Self::Toggle,
            (false, true) => Self::SelectRange,
            (true, true) => Self::AddRange,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn click(&mut self, index: usize, mode: ClickMode) {
        match (mode, self.anchor) {
            (ClickMode::SelectRange, Some(anchor)) => self.select_range(anchor, index),
            (ClickMode::AddRange, Some(anchor)) => self.add_range(anchor, index),
            (ClickMode::Toggle | ClickMode::AddRange, _) => self.toggle(index),
            (ClickMode::Select | ClickMode::SelectRange, _) => self.select_only(index),
        }
    }

    fn select_range(&mut self, anchor: usize, index: usize) {
        self.selected.clear();
        self.add_range(anchor, index);
    }

    fn add_range(&mut self, anchor: usize, index: usize) {
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

    pub fn select_all(&mut self, song_count: usize) {
        self.selected = (0..song_count).collect();
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
        selection.click(2, ClickMode::Select);
        assert_eq!(selection.indexes(), [2]);
        selection.click(5, ClickMode::SelectRange);
        assert_eq!(selection.indexes(), [2, 3, 4, 5]);
        selection.click(0, ClickMode::SelectRange);
        assert_eq!(selection.indexes(), [0, 1, 2]);
        selection.click(7, ClickMode::Toggle);
        assert_eq!(selection.indexes(), [0, 1, 2, 7]);
        selection.click(1, ClickMode::Toggle);
        assert_eq!(selection.indexes(), [0, 2, 7]);
        selection.click(3, ClickMode::AddRange);
        assert_eq!(selection.indexes(), [0, 1, 2, 3, 7]);
        selection.click(4, ClickMode::Select);
        assert_eq!(selection.indexes(), [4]);
        selection.select_all(3);
        assert_eq!(selection.len(), 3);
        selection.clear();
        assert_eq!(selection.len(), 0);
    }

    #[test]
    fn modifiers_choose_the_click_mode() {
        assert_eq!(ClickMode::from_modifiers(false, false), ClickMode::Select);
        assert_eq!(ClickMode::from_modifiers(true, false), ClickMode::Toggle);
        assert_eq!(
            ClickMode::from_modifiers(false, true),
            ClickMode::SelectRange
        );
        assert_eq!(ClickMode::from_modifiers(true, true), ClickMode::AddRange);
    }

    #[test]
    fn range_clicks_without_an_anchor_fall_back() {
        let mut selection = Selection::default();
        selection.click(3, ClickMode::SelectRange);
        assert_eq!(selection.indexes(), [3]);
        let mut selection = Selection::default();
        selection.click(4, ClickMode::AddRange);
        assert_eq!(selection.indexes(), [4]);
        selection.click(4, ClickMode::Toggle);
        assert!(selection.indexes().is_empty());
    }

    #[test]
    fn context_targets_follow_the_selection() {
        let mut selection = Selection::default();
        selection.click(1, ClickMode::Select);
        selection.click(3, ClickMode::Toggle);
        assert_eq!(selection.targets(3), [1, 3]);
        assert_eq!(selection.targets(2), [2]);
    }
}
