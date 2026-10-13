use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FolderId {
    #[default]
    Root,
    Directory(String),
}

impl FolderId {
    pub fn directory(id: impl Into<String>) -> Self {
        Self::Directory(id.into())
    }

    pub fn from_detail_id(detail_id: &str) -> Self {
        match detail_id {
            "" => Self::Root,
            id => Self::directory(id),
        }
    }

    pub fn detail_id(&self) -> &str {
        match self {
            Self::Root => "",
            Self::Directory(id) => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub cover_art_id: Option<String>,
}

impl Folder {
    pub fn folder_id(&self) -> FolderId {
        FolderId::directory(self.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_ids_round_trip() {
        assert_eq!(FolderId::from_detail_id(""), FolderId::Root);
        assert_eq!(FolderId::from_detail_id("dir-7").detail_id(), "dir-7");
        assert_eq!(FolderId::Root.detail_id(), "");
    }
}
