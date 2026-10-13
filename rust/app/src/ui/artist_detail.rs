use std::cell::RefCell;

use feishin_core::model::{Album, ArtistDetail, ItemKind, Paging, Song, SongListQuery, SongSort};
use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};

use super::artists::{CoverLru, count_label};
use super::{
    AppWindow, ArtistAlbumCard, ArtistDetailState, Context, PlayerState, TrackRow, on_ui,
    show_status,
};
use crate::controller::Command;

const IMAGE_SIZE: u32 = 360;
const ALBUM_COVER_SIZE: u32 = 200;
const ALBUM_COVER_CAPACITY: usize = 48;

/// Every song of the artist, in discography order.
pub fn artist_songs_query(artist_id: &str) -> SongListQuery {
    SongListQuery {
        sort: SongSort::Album,
        artist_ids: vec![artist_id.to_owned()],
        ..SongListQuery::default()
    }
}

/// Queues the artist's whole discography. `shuffle` turns shuffle on and starts on a random song.
pub fn play_artist(window: &AppWindow, ctx: &Context, artist_id: &str, shuffle: bool) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    if shuffle && !window.global::<PlayerState>().get_shuffle() {
        ctx.send(Command::ToggleShuffle);
    }
    let query = artist_songs_query(artist_id);
    let first = SongListQuery {
        sort: if shuffle {
            SongSort::Random
        } else {
            query.sort
        },
        ..query.clone()
    };
    let commands = ctx.clone();
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let paging = Paging {
            start_index: 0,
            limit: 1,
        };
        match server.song_list(&first, paging).await {
            Ok(page) => match page.items.into_iter().next() {
                Some(song) => commands.send(Command::PlayFromList {
                    query,
                    song_id: song.id,
                }),
                None => on_ui(&weak, |window| {
                    show_status(window, "This artist has no songs");
                }),
            },
            Err(e) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not play the artist: {e}"));
            }),
        }
    });
}

/// Turns the HTML biography servers return (Last.fm summaries) into plain paragraphs.
pub fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end].to_ascii_lowercase();
        let name = tag.trim_start_matches('/').split([' ', '/']).next();
        if matches!(name, Some("br" | "p" | "div" | "li")) {
            text.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    text.push_str(rest);
    let text = [
        ("&nbsp;", " "),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
    ]
    .iter()
    .fold(text, |text, (entity, value)| text.replace(entity, value));
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn info_line(detail: &ArtistDetail) -> String {
    let albums = detail
        .artist
        .album_count
        .or_else(|| u32::try_from(detail.albums.len()).ok());
    let songs = detail.artist.song_count.or_else(|| {
        let total: u32 = detail.albums.iter().map(|album| album.song_count).sum();
        (total > 0).then_some(total)
    });
    [
        count_label(albums, "album", "albums"),
        count_label(songs, "song", "songs"),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" • ")
}

fn album_card(album: &Album) -> ArtistAlbumCard {
    ArtistAlbumCard {
        id: album.id.as_str().into(),
        name: album.name.as_str().into(),
        year: album.year.map(|y| y.to_string()).unwrap_or_default().into(),
        image: Image::default(),
        has_image: false,
    }
}

/// The artist on screen; responses for an older `generation` are dropped.
struct Detail {
    generation: u64,
    id: String,
    loaded: bool,
    favorite: bool,
    top_songs: Vec<Song>,
    album_covers: Vec<Option<String>>,
    covers: CoverLru,
}

impl Default for Detail {
    fn default() -> Self {
        Self {
            generation: 0,
            id: String::new(),
            loaded: false,
            favorite: false,
            top_songs: Vec::new(),
            album_covers: Vec::new(),
            covers: CoverLru::new(ALBUM_COVER_CAPACITY),
        }
    }
}

thread_local! {
    static DETAIL: RefCell<Detail> = RefCell::default();
}

/// Starts showing `id`; returns the new generation, or `None` when it is already shown or loading.
fn begin(id: &str) -> Option<u64> {
    DETAIL.with_borrow_mut(|detail| {
        if id.is_empty() || detail.id == id {
            return None;
        }
        *detail = Detail {
            generation: detail.generation + 1,
            id: id.to_owned(),
            ..Detail::default()
        };
        Some(detail.generation)
    })
}

fn is_current(generation: u64) -> bool {
    DETAIL.with_borrow(|detail| detail.generation == generation)
}

fn set_album_image(window: &AppWindow, index: usize, image: Option<Image>) {
    let model = window.global::<ArtistDetailState>().get_albums();
    if let Some(mut card) = model.row_data(index) {
        card.has_image = image.is_some();
        card.image = image.unwrap_or_default();
        model.set_row_data(index, card);
    }
}

fn clear(window: &AppWindow) {
    let state = window.global::<ArtistDetailState>();
    state.set_error("".into());
    state.set_name("".into());
    state.set_info("".into());
    state.set_image(Image::default());
    state.set_has_image(false);
    state.set_favorite(false);
    state.set_biography("".into());
    state.set_bio_expanded(false);
    state.set_top_expanded(false);
    state.set_top_songs(ModelRc::default());
    state.set_albums(ModelRc::default());
}

fn show(window: &AppWindow, detail: ArtistDetail, image: Option<Image>) {
    let state = window.global::<ArtistDetailState>();
    state.set_loading(false);
    state.set_name(detail.artist.name.as_str().into());
    state.set_info(info_line(&detail).into());
    state.set_favorite(detail.artist.favorite);
    state.set_biography(plain_text(detail.artist.biography.as_deref().unwrap_or_default()).into());
    if let Some(image) = image {
        state.set_image(image);
        state.set_has_image(true);
    }
    let rows: Vec<TrackRow> = detail
        .top_songs
        .iter()
        .enumerate()
        .map(|(i, song)| super::tracks::track_row(song, i))
        .collect();
    state.set_top_songs(ModelRc::new(VecModel::from(rows)));
    let albums: Vec<ArtistAlbumCard> = detail.albums.iter().map(album_card).collect();
    DETAIL.with_borrow_mut(|current| {
        current.loaded = true;
        current.favorite = detail.artist.favorite;
        current.album_covers = detail
            .albums
            .iter()
            .map(|album| album.cover_art_id.clone())
            .collect();
        current.top_songs = detail.top_songs;
    });
    state.set_albums(ModelRc::new(VecModel::from(albums)));
}

fn load(window: &AppWindow, ctx: &Context, id: &str) {
    let Some(generation) = begin(id) else {
        return;
    };
    clear(window);
    window.global::<ArtistDetailState>().set_loading(true);
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let covers = ctx.covers.clone();
    let id = id.to_owned();
    ctx.rt.spawn(async move {
        let detail = server.artist(&id).await;
        let cover_id = detail
            .as_ref()
            .ok()
            .and_then(|d| d.artist.cover_art_id.clone());
        let image = match cover_id {
            Some(cover_id) => covers
                .cover(server.as_ref(), &cover_id, IMAGE_SIZE)
                .await
                .inspect_err(|e| tracing::debug!("artist image {cover_id}: {e:#}"))
                .ok(),
            None => None,
        };
        on_ui(&weak, move |window| {
            if !is_current(generation) {
                return;
            }
            match detail {
                Ok(detail) => {
                    let image = image.as_deref().and_then(super::player::to_image);
                    show(window, detail, image);
                }
                Err(e) => {
                    DETAIL.with_borrow_mut(|detail| detail.id.clear());
                    let state = window.global::<ArtistDetailState>();
                    state.set_loading(false);
                    state.set_error(format!("Could not load the artist: {e}").into());
                }
            }
        });
    });
}

fn load_album_cover(window: &AppWindow, ctx: &Context, index: usize) {
    let request = DETAIL.with_borrow_mut(|detail| {
        let cover_id = detail.album_covers.get(index).cloned().flatten()?;
        detail
            .covers
            .begin(index)
            .then_some((detail.generation, cover_id))
    });
    let (Some((generation, cover_id)), Some(server)) = (request, ctx.session.get()) else {
        return;
    };
    let weak = window.as_weak();
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        let cover = covers
            .cover(server.as_ref(), &cover_id, ALBUM_COVER_SIZE)
            .await;
        if let Err(e) = &cover {
            tracing::debug!("album cover {cover_id}: {e:#}");
        }
        on_ui(&weak, move |window| {
            let image = cover.ok().and_then(|cover| super::player::to_image(&cover));
            apply_album_cover(window, generation, index, image);
        });
    });
}

fn apply_album_cover(window: &AppWindow, generation: u64, index: usize, image: Option<Image>) {
    let evicted = DETAIL.with_borrow_mut(|detail| {
        if detail.generation != generation {
            return None;
        }
        match &image {
            Some(image) => Some(detail.covers.insert(index, image.clone())),
            None => {
                detail.covers.failed(index);
                None
            }
        }
    });
    let Some(evicted) = evicted else {
        return;
    };
    for old in evicted {
        set_album_image(window, old, None);
    }
    set_album_image(window, index, image);
}

fn toggle_favorite(window: &AppWindow, ctx: &Context) {
    let Some((generation, id, favorite)) = DETAIL.with_borrow_mut(|detail| {
        if !detail.loaded {
            return None;
        }
        detail.favorite = !detail.favorite;
        Some((detail.generation, detail.id.clone(), detail.favorite))
    }) else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    window.global::<ArtistDetailState>().set_favorite(favorite);
    super::artists::set_favorite(window, &id, favorite);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let ids = [id.clone()];
        if let Err(e) = server.set_favorite(ItemKind::Artist, &ids, favorite).await {
            on_ui(&weak, move |window| {
                super::artists::set_favorite(window, &id, !favorite);
                if is_current(generation) {
                    DETAIL.with_borrow_mut(|detail| detail.favorite = !favorite);
                    window.global::<ArtistDetailState>().set_favorite(!favorite);
                }
                show_status(window, &format!("Could not update the favorite: {e}"));
            });
        }
    });
}

fn play_album(window: &AppWindow, ctx: &Context, album_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let commands = ctx.clone();
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        match server.album(&album_id).await {
            Ok(album) => commands.send(Command::PlaySongs {
                songs: album.songs,
                song_id: None,
            }),
            Err(e) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not play the album: {e}"));
            }),
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<ArtistDetailState>();

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    state.on_load(move |id| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx, &id);
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play(move |shuffle| {
        let id = DETAIL.with_borrow(|detail| detail.id.clone());
        if let (Some(window), false) = (weak.upgrade(), id.is_empty()) {
            play_artist(&window, &play_ctx, &id, shuffle);
        }
    });

    let weak = window.as_weak();
    let favorite_ctx = ctx.clone();
    state.on_toggle_favorite(move || {
        if let Some(window) = weak.upgrade() {
            toggle_favorite(&window, &favorite_ctx);
        }
    });

    let top_ctx = ctx.clone();
    state.on_play_top(move |song_id| {
        let songs = DETAIL.with_borrow(|detail| detail.top_songs.clone());
        if !songs.is_empty() {
            top_ctx.send(Command::PlaySongs {
                songs,
                song_id: Some(song_id.to_string()),
            });
        }
    });

    let weak = window.as_weak();
    let album_ctx = ctx.clone();
    state.on_play_album(move |album_id| {
        if let Some(window) = weak.upgrade() {
            play_album(&window, &album_ctx, album_id.to_string());
        }
    });

    let weak = window.as_weak();
    let cover_ctx = ctx.clone();
    state.on_album_shown(move |index| {
        if let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) {
            load_album_cover(&window, &cover_ctx, index);
        }
    });
}

#[cfg(test)]
mod tests {
    use feishin_api::MusicServer;

    use super::*;
    use crate::demo::FakeServer;

    #[test]
    fn biography_html_becomes_plain_paragraphs() {
        let html = "<p>Ada  Vale is a <i>folk</i> artist.</p><p>Small &amp; noisy rooms.<br/>\
                    <a href=\"https://www.last.fm\">Read more on Last.fm</a></p>";
        assert_eq!(
            plain_text(html),
            "Ada Vale is a folk artist.\n\nSmall & noisy rooms.\n\nRead more on Last.fm"
        );
        assert_eq!(plain_text("plain text"), "plain text");
        assert_eq!(plain_text(""), "");
        assert_eq!(plain_text("broken <b"), "broken");
    }

    #[test]
    fn play_all_queries_every_song_of_the_artist() {
        let query = artist_songs_query("artist-7");
        assert_eq!(query.artist_ids, ["artist-7"]);
        assert_eq!(query.sort, SongSort::Album);
        assert!(query.search.is_none() && query.album_ids.is_empty());
    }

    #[test]
    fn generations_guard_navigation() {
        let first = begin("artist-1");
        assert_eq!(begin("artist-1"), None);
        let second = begin("artist-2");
        assert!(first.is_some() && second > first);
        assert!(first.is_some_and(|g| !is_current(g)));
        assert!(second.is_some_and(is_current));
        assert_eq!(begin(""), None);
    }

    #[tokio::test]
    async fn demo_artist_detail_has_everything_the_view_shows() {
        let server = FakeServer::new(2000);
        let artist_id = server
            .album("album-0")
            .await
            .ok()
            .and_then(|album| album.album.album_artists.first().map(|a| a.id.clone()))
            .unwrap_or_default();
        let Ok(detail) = server.artist(&artist_id).await else {
            panic!("demo artist {artist_id} missing");
        };
        assert!(!detail.albums.is_empty());
        assert!(!detail.top_songs.is_empty());
        assert!(
            detail
                .top_songs
                .windows(2)
                .all(|pair| pair[0].play_count >= pair[1].play_count)
        );
        assert!(detail.artist.cover_art_id.is_some());
        assert!(!plain_text(detail.artist.biography.as_deref().unwrap_or_default()).is_empty());
        let info = info_line(&detail);
        assert!(info.contains("album") && info.contains("songs"), "{info}");

        let query = artist_songs_query(&artist_id);
        let songs = server
            .song_list(
                &query,
                Paging {
                    start_index: 0,
                    limit: 500,
                },
            )
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        let total: u32 = detail.albums.iter().map(|a| a.song_count).sum();
        assert_eq!(songs.len(), total as usize);
    }
}
