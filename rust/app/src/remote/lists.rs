use feishin_core::model::FolderId;
use feishin_remote::{ListPlacement, RemoteList};

use crate::list_stream::{Placement, PlaylistId, SongSource};

pub fn remote_list_of(source: SongSource) -> RemoteList {
    match source {
        SongSource::Library(query) => RemoteList::Library { query },
        SongSource::Playlist(PlaylistId(id)) => RemoteList::Playlist { id },
        SongSource::Folder(folder) => RemoteList::Folder {
            id: folder_detail_id(folder),
        },
        SongSource::FolderTree(folder) => RemoteList::FolderTree {
            id: folder_detail_id(folder),
        },
    }
}

pub fn song_source_of(list: RemoteList) -> SongSource {
    match list {
        RemoteList::Library { query } => SongSource::Library(query),
        RemoteList::Playlist { id } => SongSource::Playlist(PlaylistId(id)),
        RemoteList::Folder { id } => SongSource::Folder(FolderId::from_detail_id(&id)),
        RemoteList::FolderTree { id } => SongSource::FolderTree(FolderId::from_detail_id(&id)),
    }
}

fn folder_detail_id(folder: FolderId) -> String {
    match folder {
        FolderId::Root => String::new(),
        FolderId::Directory(id) => id,
    }
}

pub fn remote_placement_of(placement: Placement) -> ListPlacement {
    match placement {
        Placement::Now => ListPlacement::Now,
        Placement::Next => ListPlacement::Next,
        Placement::Last => ListPlacement::Last,
    }
}

pub fn placement_of(placement: ListPlacement) -> Placement {
    match placement {
        ListPlacement::Now => Placement::Now,
        ListPlacement::Next => Placement::Next,
        ListPlacement::Last => Placement::Last,
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::SongListQuery;

    use super::*;

    #[test]
    fn every_list_survives_the_trip_to_another_device() {
        let sources = [
            SongSource::Library(SongListQuery::default()),
            SongSource::Playlist(PlaylistId("p1".into())),
            SongSource::Folder(FolderId::Root),
            SongSource::FolderTree(FolderId::directory("d1")),
        ];
        for source in sources {
            assert_eq!(song_source_of(remote_list_of(source.clone())), source);
        }
        for placement in [Placement::Now, Placement::Next, Placement::Last] {
            assert_eq!(placement_of(remote_placement_of(placement)), placement);
        }
    }
}
