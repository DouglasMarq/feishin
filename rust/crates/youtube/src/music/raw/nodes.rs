use reqwest::Url;
use serde_json::Value;

use crate::MILLISECONDS_PER_SECOND;

const RESIZABLE_THUMBNAIL_HOSTS: [&str; 2] = ["googleusercontent.com", "ggpht.com"];
const RUN_SEPARATOR: &str = " • ";
const THUMBNAIL_SIZE: u32 = 300;
const SECONDS_PER_MINUTE: u64 = 60;
const DURATION_PARTS: std::ops::RangeInclusive<usize> = 2..=3;
const PAGE_TYPE_POINTER: &str =
    "/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType";

pub(super) const ALBUM_PAGE: &str = "MUSIC_PAGE_TYPE_ALBUM";
pub(super) const PLAYLIST_PAGE: &str = "MUSIC_PAGE_TYPE_PLAYLIST";
const ARTIST_PAGES: [&str; 2] = ["MUSIC_PAGE_TYPE_ARTIST", "MUSIC_PAGE_TYPE_USER_CHANNEL"];

pub(super) fn array<'node>(node: &'node Value, pointer: &str) -> &'node [Value] {
    node.pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(super) fn string<'node>(node: &'node Value, pointer: &str) -> Option<&'node str> {
    node.pointer(pointer).and_then(Value::as_str)
}

pub(super) fn runs(text_node: Option<&Value>) -> &[Value] {
    text_node
        .and_then(|node| node.get("runs"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(super) fn run_text(run: &Value) -> &str {
    run.get("text").and_then(Value::as_str).unwrap_or_default()
}

pub(super) fn text_of(text_node: Option<&Value>) -> String {
    let joined: String = runs(text_node).iter().map(run_text).collect();
    match joined.is_empty() {
        true => text_node
            .and_then(|node| node.get("simpleText"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        false => joined,
    }
}

pub(super) fn run_groups(runs: &[Value]) -> Vec<Vec<&Value>> {
    let mut groups = vec![Vec::new()];
    for run in runs {
        match (run_text(run) == RUN_SEPARATOR, groups.last_mut()) {
            (true, _) => groups.push(Vec::new()),
            (false, Some(group)) => group.push(run),
            (false, None) => {}
        }
    }
    groups.retain(|group| !group.is_empty());
    groups
}

pub(super) fn group_text(group: &[&Value]) -> String {
    group.iter().map(|run| run_text(run)).collect::<String>()
}

pub(super) fn browse_endpoint(node: &Value) -> Option<&Value> {
    node.get("browseEndpoint")
}

pub(super) fn browse_id(endpoint: &Value) -> Option<&str> {
    browse_endpoint(endpoint)?.get("browseId")?.as_str()
}

pub(super) fn page_type(endpoint: &Value) -> Option<&str> {
    browse_endpoint(endpoint)?
        .pointer(PAGE_TYPE_POINTER)?
        .as_str()
}

pub(super) fn watch_video_id(endpoint: &Value) -> Option<&str> {
    endpoint.pointer("/watchEndpoint/videoId")?.as_str()
}

pub(super) fn run_page_type(run: &Value) -> Option<&str> {
    run.get("navigationEndpoint").and_then(page_type)
}

pub(super) fn is_artist_run(run: &Value) -> bool {
    run_page_type(run).is_some_and(|page| ARTIST_PAGES.contains(&page))
}

pub(super) fn is_album_run(run: &Value) -> bool {
    run_page_type(run) == Some(ALBUM_PAGE)
}

pub(super) fn parse_duration(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if !DURATION_PARTS.contains(&parts.len()) {
        return None;
    }
    parts
        .iter()
        .try_fold(0_u64, |total, part| {
            let valid = !part.is_empty() && part.chars().all(|digit| digit.is_ascii_digit());
            let value: u64 = valid.then(|| part.parse().ok()).flatten()?;
            total.checked_mul(SECONDS_PER_MINUTE)?.checked_add(value)
        })
        .and_then(|seconds| seconds.checked_mul(MILLISECONDS_PER_SECOND))
}

pub(super) fn best_thumbnail(thumbnail_holder: Option<&Value>) -> Option<String> {
    let thumbnails = thumbnail_holder?.get("thumbnails")?.as_array()?;
    let widest = thumbnails.iter().max_by_key(|thumbnail| {
        thumbnail
            .get("width")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    })?;
    let url = widest.get("url")?.as_str()?;
    Some(sized_thumbnail(url, THUMBNAIL_SIZE))
}

fn sized_thumbnail(url: &str, size: u32) -> String {
    let Some((base, options)) = url.rsplit_once('=').filter(|_| is_resizable_thumbnail(url)) else {
        return url.to_owned();
    };
    let kept: String = options
        .split('-')
        .filter(|option| !is_size_option(option))
        .map(|option| format!("-{option}"))
        .collect();
    format!("{base}=w{size}-h{size}{kept}")
}

fn is_resizable_thumbnail(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .as_ref()
        .and_then(Url::host_str)
        .is_some_and(|host| {
            RESIZABLE_THUMBNAIL_HOSTS
                .iter()
                .any(|resizable| host == *resizable || host.ends_with(&format!(".{resizable}")))
        })
}

fn is_size_option(option: &str) -> bool {
    let mut characters = option.chars();
    let prefix = characters.next();
    let digits = characters.as_str();
    matches!(prefix, Some('w' | 'h' | 's'))
        && !digits.is_empty()
        && digits.chars().all(|digit| digit.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn durations_parse_minutes_and_hours() {
        assert_eq!(parse_duration("3:25"), Some(205_000));
        assert_eq!(parse_duration("1:02:03"), Some(3_723_000));
        assert_eq!(parse_duration("Tocou 1,9 mi vezes"), None);
        assert_eq!(parse_duration("3:"), None);
        assert_eq!(parse_duration("12"), None);
        assert_eq!(parse_duration("9999999999999999999:59"), None);
    }

    #[test]
    fn googleusercontent_thumbnails_are_resized() {
        let holder = json!({"thumbnails": [
            {"url": "https://lh3.googleusercontent.com/abc=w60-h60-l90-rj", "width": 60},
            {"url": "https://lh3.googleusercontent.com/abc=w120-h120-l90-rj", "width": 120}
        ]});
        assert_eq!(
            best_thumbnail(Some(&holder)).as_deref(),
            Some("https://lh3.googleusercontent.com/abc=w300-h300-l90-rj")
        );
        let channel =
            json!({"thumbnails": [{"url": "https://yt3.ggpht.com/x=s192", "width": 192}]});
        assert_eq!(
            best_thumbnail(Some(&channel)).as_deref(),
            Some("https://yt3.ggpht.com/x=w300-h300")
        );
    }

    #[test]
    fn given_a_thumbnail_host_that_only_mentions_google_it_is_kept_as_it_is() {
        let lookalike = "https://images.example/googleusercontent.com/abc=w60-h60";
        let holder = json!({"thumbnails": [{"url": lookalike, "width": 60}]});
        assert_eq!(best_thumbnail(Some(&holder)).as_deref(), Some(lookalike));
        let suffix = "https://notggpht.com/abc=s60";
        let holder = json!({"thumbnails": [{"url": suffix, "width": 60}]});
        assert_eq!(best_thumbnail(Some(&holder)).as_deref(), Some(suffix));
    }

    #[test]
    fn video_thumbnails_are_kept_as_they_are() {
        let holder = json!({"thumbnails": [{"url": "https://i.ytimg.com/vi/a/hqdefault.jpg?sqp=x=y", "width": 400}]});
        assert_eq!(
            best_thumbnail(Some(&holder)).as_deref(),
            Some("https://i.ytimg.com/vi/a/hqdefault.jpg?sqp=x=y")
        );
        assert_eq!(best_thumbnail(Some(&json!({}))), None);
    }

    #[test]
    fn runs_split_on_bullets() {
        let text = json!({"runs": [{"text": "A"}, {"text": " & "}, {"text": "B"}, {"text": " • "}, {"text": "3:10"}]});
        let groups = run_groups(runs(Some(&text)));
        let texts: Vec<String> = groups.iter().map(|group| group_text(group)).collect();
        assert_eq!(texts, ["A & B", "3:10"]);
        assert_eq!(text_of(Some(&text)), "A & B • 3:10");
        assert_eq!(text_of(Some(&json!({"simpleText": "Plain"}))), "Plain");
    }
}
