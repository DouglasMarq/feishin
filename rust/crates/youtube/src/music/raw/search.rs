use serde_json::Value;

use super::items::{SongDefaults, list_item_song};
use super::nodes::array;
use crate::music::model::MusicSong;

const SEARCH_SECTIONS: &str =
    "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";

pub(crate) fn search_songs(response: &Value) -> Vec<MusicSong> {
    let defaults = SongDefaults::default();
    array(response, SEARCH_SECTIONS)
        .iter()
        .flat_map(|section| array(section, "/musicShelfRenderer/contents"))
        .filter_map(|item| item.get("musicResponsiveListItemRenderer"))
        .filter_map(|renderer| list_item_song(renderer, &defaults))
        .collect()
}

pub(crate) fn has_search_sections(response: &Value) -> bool {
    response.pointer(SEARCH_SECTIONS).is_some()
}
