use feishin_storage::appearance::ThemeId;
use slint::{ComponentHandle, Model};

use crate::ui::{AppWindow, Palettes};

struct CatalogEntry {
    id: ThemeId,
}

pub(super) struct PaletteCatalog {
    entries: Vec<CatalogEntry>,
}

impl PaletteCatalog {
    pub(super) fn from_window(window: &AppWindow) -> Self {
        let entries = window
            .global::<Palettes>()
            .get_all()
            .iter()
            .map(|palette| CatalogEntry {
                id: ThemeId::new(palette.id.as_str()),
            })
            .collect();
        Self { entries }
    }

    pub(super) fn index_of(&self, id: &ThemeId) -> Option<usize> {
        self.entries.iter().position(|entry| &entry.id == id)
    }

    pub(super) fn index_or(&self, id: &ThemeId, fallback: &ThemeId) -> usize {
        self.index_of(id)
            .or_else(|| self.index_of(fallback))
            .unwrap_or_default()
    }

    pub(super) fn id_at(&self, index: i32) -> Option<ThemeId> {
        let entry = self.entries.get(usize::try_from(index).ok()?)?;
        Some(entry.id.clone())
    }

    #[cfg(test)]
    pub(super) fn ids(&self) -> impl Iterator<Item = &ThemeId> {
        self.entries.iter().map(|entry| &entry.id)
    }
}
