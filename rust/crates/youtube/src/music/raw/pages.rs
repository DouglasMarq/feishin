use serde_json::Value;

use super::items::shelf_item;
use super::nodes::{array, browse_endpoint, browse_id, text_of};
use crate::music::model::{MoodCategory, MusicShelf};

const SECTIONS: &str = "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";
const CAROUSEL_TITLE: &str = "/header/musicCarouselShelfBasicHeaderRenderer/title";
const GRID_TITLE: &str = "/header/gridHeaderRenderer/title";
pub(crate) const MOODS_CATEGORY_BROWSE_ID: &str = "FEmusic_moods_and_genres_category";

pub(crate) fn browse_shelves(response: &Value) -> Vec<MusicShelf> {
    array(response, SECTIONS)
        .iter()
        .filter_map(section_shelf)
        .filter(|shelf| !shelf.items.is_empty())
        .collect()
}

pub(crate) fn has_sections(response: &Value) -> bool {
    response.pointer(SECTIONS).is_some()
}

fn section_shelf(section: &Value) -> Option<MusicShelf> {
    if let Some(carousel) = section.get("musicCarouselShelfRenderer") {
        let title = text_of(carousel.pointer(CAROUSEL_TITLE));
        return Some(shelf(title, array(carousel, "/contents")));
    }
    if let Some(grid) = section.get("gridRenderer") {
        let title = text_of(grid.pointer(GRID_TITLE));
        return Some(shelf(title, array(grid, "/items")));
    }
    section
        .get("musicShelfRenderer")
        .map(|list| shelf(text_of(list.get("title")), array(list, "/contents")))
}

fn shelf(title: String, items: &[Value]) -> MusicShelf {
    MusicShelf {
        title,
        items: items.iter().filter_map(shelf_item).collect(),
    }
}

pub(crate) fn mood_categories(response: &Value) -> Vec<MoodCategory> {
    let mut categories: Vec<MoodCategory> = Vec::new();
    for category in array(response, SECTIONS)
        .iter()
        .flat_map(section_items)
        .filter_map(mood_category)
    {
        if !categories
            .iter()
            .any(|known| known.params == category.params)
        {
            categories.push(category);
        }
    }
    categories
}

fn section_items(section: &Value) -> &[Value] {
    match section.get("musicCarouselShelfRenderer") {
        Some(carousel) => array(carousel, "/contents"),
        None => array(section, "/gridRenderer/items"),
    }
}

fn mood_category(item: &Value) -> Option<MoodCategory> {
    let button = item.get("musicNavigationButtonRenderer")?;
    let endpoint = button.get("clickCommand")?;
    if browse_id(endpoint)? != MOODS_CATEGORY_BROWSE_ID {
        return None;
    }
    let params = browse_endpoint(endpoint)?.get("params")?.as_str()?;
    let title = text_of(button.get("buttonText"));
    (!title.is_empty()).then(|| MoodCategory {
        title,
        params: params.to_owned(),
    })
}
