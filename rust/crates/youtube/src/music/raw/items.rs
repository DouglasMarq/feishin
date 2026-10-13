use serde_json::Value;

use super::nodes::{
    ALBUM_PAGE, PLAYLIST_PAGE, array, best_thumbnail, browse_id, group_text, is_album_run,
    is_artist_run, page_type, parse_duration, run_groups, run_text, runs, string, text_of,
    watch_video_id,
};
use crate::is_video_id;
use crate::music::model::{CollectionKind, MusicCollection, MusicSong, ShelfItem, is_browse_id};

const FLEX_TEXT: &str = "/musicResponsiveListItemFlexColumnRenderer/text";
const FIXED_TEXT: &str = "/musicResponsiveListItemFixedColumnRenderer/text";
const LIST_THUMBNAIL: &str = "/thumbnail/musicThumbnailRenderer/thumbnail";
const CARD_THUMBNAIL: &str = "/thumbnailRenderer/musicThumbnailRenderer/thumbnail";
const TITLE_ENDPOINT: &str =
    "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint";
const OVERLAY_ENDPOINT: &str = "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint";

#[derive(Debug, Clone, Default)]
pub(super) struct SongDefaults {
    pub(super) album: Option<String>,
    pub(super) artists: Vec<String>,
    pub(super) thumbnail_url: Option<String>,
}

pub(super) fn shelf_item(item: &Value) -> Option<ShelfItem> {
    if let Some(renderer) = item.get("musicResponsiveListItemRenderer") {
        return list_item_song(renderer, &SongDefaults::default()).map(ShelfItem::Song);
    }
    item.get("musicTwoRowItemRenderer").and_then(card_item)
}

pub(super) fn list_item_song(renderer: &Value, defaults: &SongDefaults) -> Option<MusicSong> {
    let video_id = list_item_video_id(renderer)?;
    let columns: Vec<Option<&Value>> = array(renderer, "/flexColumns")
        .iter()
        .map(|column| column.pointer(FLEX_TEXT))
        .collect();
    let title = text_of(columns.first().copied().flatten());
    if title.is_empty() {
        return None;
    }
    let detail_runs: Vec<&Value> = columns
        .iter()
        .skip(1)
        .flat_map(|column| runs(*column))
        .collect();
    let detail_groups = run_groups(runs(columns.get(1).copied().flatten()));
    Some(MusicSong {
        video_id: video_id.to_owned(),
        title,
        artists: artists_of(&detail_runs, &detail_groups)
            .unwrap_or_else(|| defaults.artists.clone()),
        album: album_of(&detail_runs).or_else(|| defaults.album.clone()),
        duration_ms: fixed_duration(renderer).or_else(|| groups_duration(&detail_groups)),
        thumbnail_url: best_thumbnail(renderer.pointer(LIST_THUMBNAIL))
            .or_else(|| defaults.thumbnail_url.clone()),
    })
}

fn list_item_video_id(renderer: &Value) -> Option<&str> {
    [
        string(renderer, "/playlistItemData/videoId"),
        renderer.pointer(TITLE_ENDPOINT).and_then(watch_video_id),
        renderer.pointer(OVERLAY_ENDPOINT).and_then(watch_video_id),
    ]
    .into_iter()
    .flatten()
    .find(|video_id| is_video_id(video_id))
}

fn artists_of(detail_runs: &[&Value], detail_groups: &[Vec<&Value>]) -> Option<Vec<String>> {
    let linked: Vec<String> = detail_runs
        .iter()
        .filter(|run| is_artist_run(run))
        .map(|run| run_text(run).to_owned())
        .collect();
    if !linked.is_empty() {
        return Some(linked);
    }
    let first_group = detail_groups.first()?;
    let text = group_text(first_group);
    let plain = !first_group.iter().any(|run| is_album_run(run))
        && parse_duration(&text).is_none()
        && !text.trim().is_empty();
    plain.then(|| vec![text.trim().to_owned()])
}

fn album_of(detail_runs: &[&Value]) -> Option<String> {
    detail_runs
        .iter()
        .find(|run| is_album_run(run))
        .map(|run| run_text(run).to_owned())
}

fn fixed_duration(renderer: &Value) -> Option<u64> {
    array(renderer, "/fixedColumns")
        .iter()
        .filter_map(|column| column.pointer(FIXED_TEXT))
        .find_map(|text| parse_duration(&text_of(Some(text))))
}

fn groups_duration(detail_groups: &[Vec<&Value>]) -> Option<u64> {
    detail_groups
        .iter()
        .find_map(|group| parse_duration(&group_text(group)))
}

fn card_item(renderer: &Value) -> Option<ShelfItem> {
    let endpoint = renderer.get("navigationEndpoint")?;
    let title = text_of(renderer.get("title"));
    if title.is_empty() {
        return None;
    }
    let thumbnail_url = best_thumbnail(renderer.pointer(CARD_THUMBNAIL));
    if let Some(video_id) = watch_video_id(endpoint).filter(|video_id| is_video_id(video_id)) {
        return Some(ShelfItem::Song(card_song(
            renderer,
            video_id,
            title,
            thumbnail_url,
        )));
    }
    let kind = match page_type(endpoint)? {
        ALBUM_PAGE => CollectionKind::Album,
        PLAYLIST_PAGE => CollectionKind::Playlist,
        _ => return None,
    };
    let browse_id = browse_id(endpoint).filter(|browse_id| is_browse_id(browse_id))?;
    Some(ShelfItem::Collection(MusicCollection {
        browse_id: browse_id.to_owned(),
        kind,
        title,
        subtitle: text_of(renderer.get("subtitle")),
        thumbnail_url,
    }))
}

fn card_song(
    renderer: &Value,
    video_id: &str,
    title: String,
    thumbnail_url: Option<String>,
) -> MusicSong {
    let subtitle_runs: Vec<&Value> = runs(renderer.get("subtitle")).iter().collect();
    let subtitle_groups = run_groups(runs(renderer.get("subtitle")));
    MusicSong {
        video_id: video_id.to_owned(),
        title,
        artists: artists_of(&subtitle_runs, &subtitle_groups).unwrap_or_default(),
        album: album_of(&subtitle_runs),
        duration_ms: groups_duration(&subtitle_groups),
        thumbnail_url,
    }
}
