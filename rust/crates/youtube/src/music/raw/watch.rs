use serde_json::Value;

use super::items::{album_of, artists_of};
use super::nodes::{array, best_thumbnail, parse_duration, run_groups, runs, string, text_of};
use crate::is_video_id;
use crate::music::model::MusicSong;

const WATCH_PANEL: &str = "/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer/content/playlistPanelRenderer/contents";
const WRAPPED_VIDEO: &str =
    "/playlistPanelVideoWrapperRenderer/primaryRenderer/playlistPanelVideoRenderer";

pub(crate) fn watch_songs(response: &Value) -> Vec<MusicSong> {
    array(response, WATCH_PANEL)
        .iter()
        .filter_map(panel_video)
        .filter_map(panel_song)
        .collect()
}

pub(crate) fn has_watch_panel(response: &Value) -> bool {
    response.pointer(WATCH_PANEL).is_some()
}

fn panel_video(item: &Value) -> Option<&Value> {
    item.get("playlistPanelVideoRenderer")
        .or_else(|| item.pointer(WRAPPED_VIDEO))
}

fn panel_song(video: &Value) -> Option<MusicSong> {
    let video_id = string(video, "/videoId").filter(|video_id| is_video_id(video_id))?;
    let title = text_of(video.get("title"));
    if title.is_empty() {
        return None;
    }
    let byline: Vec<&Value> = runs(video.get("longBylineText")).iter().collect();
    let byline_groups = run_groups(runs(video.get("longBylineText")));
    Some(MusicSong {
        video_id: video_id.to_owned(),
        title,
        artists: artists_of(&byline, &byline_groups).unwrap_or_default(),
        album: album_of(&byline),
        duration_ms: parse_duration(&text_of(video.get("lengthText"))),
        thumbnail_url: best_thumbnail(video.get("thumbnail")),
    })
}
