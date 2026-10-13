use serde_json::Value;

use super::items::{SongDefaults, list_item_song};
use super::nodes::{array, best_thumbnail, text_of};
use crate::music::model::{CollectionKind, CollectionPage};

const HEADER_SECTIONS: &str = "/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0";
const HEADER: &str = "/musicResponsiveHeaderRenderer";
const EDITABLE_HEADER: &str =
    "/musicEditablePlaylistDetailHeaderRenderer/header/musicResponsiveHeaderRenderer";
const TRACK_SECTIONS: &str =
    "/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents";
const HEADER_THUMBNAIL: &str = "/thumbnail/musicThumbnailRenderer/thumbnail";
const DETAIL_SEPARATOR: &str = " • ";

pub(crate) fn collection_page(response: &Value, kind: CollectionKind) -> Option<CollectionPage> {
    let header = response.pointer(HEADER_SECTIONS).and_then(|section| {
        section
            .pointer(HEADER)
            .or_else(|| section.pointer(EDITABLE_HEADER))
    });
    let title = text_of(header.and_then(|header| header.get("title")));
    let owner = text_of(header.and_then(|header| header.get("straplineTextOne")));
    let details = text_of(header.and_then(|header| header.get("subtitle")));
    let thumbnail_url = best_thumbnail(header.and_then(|header| header.pointer(HEADER_THUMBNAIL)));
    let defaults = song_defaults(kind, &title, &owner, thumbnail_url.clone());
    let songs: Vec<_> = array(response, TRACK_SECTIONS)
        .iter()
        .flat_map(track_rows)
        .filter_map(|row| row.get("musicResponsiveListItemRenderer"))
        .filter_map(|renderer| list_item_song(renderer, &defaults))
        .collect();
    if header.is_none() && songs.is_empty() {
        return None;
    }
    Some(CollectionPage {
        title,
        subtitle: joined_details(&[&owner, &details]),
        thumbnail_url,
        songs,
    })
}

fn song_defaults(
    kind: CollectionKind,
    title: &str,
    owner: &str,
    thumbnail_url: Option<String>,
) -> SongDefaults {
    match kind {
        CollectionKind::Album => SongDefaults {
            album: Some(title.to_owned()).filter(|title| !title.is_empty()),
            artists: Some(owner.to_owned())
                .filter(|owner| !owner.is_empty())
                .into_iter()
                .collect(),
            thumbnail_url,
        },
        CollectionKind::Playlist => SongDefaults::default(),
    }
}

fn track_rows(section: &Value) -> &[Value] {
    match section.get("musicShelfRenderer") {
        Some(shelf) => array(shelf, "/contents"),
        None => array(section, "/musicPlaylistShelfRenderer/contents"),
    }
}

fn joined_details(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(DETAIL_SEPARATOR)
}
