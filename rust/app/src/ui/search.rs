use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fmt::Display;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use feishin_api::MusicServer;
use feishin_core::model::{
    Album, AlbumListQuery, Artist, ArtistListQuery, ArtistRef, Paging, SearchResults, Song,
    SongListQuery,
};
use slint::{ComponentHandle, Model, ModelRc, ModelTracker, Timer, TimerMode, VecModel};

use super::{AppWindow, Context, SearchCard, SearchKind, SearchState, TrackRow, on_ui};
use crate::controller::Command;
use crate::ui::player::to_image;
use crate::ui::tracks::track_row;

pub const SEARCH_LIMIT: usize = 20;
const MORE_PAGE: usize = 50;
const DEBOUNCE: Duration = Duration::from_millis(250);
const COVER_SIZE: u32 = 192;
const MAX_COVERS: usize = 48;

fn normalize(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn song_query(search: &str) -> SongListQuery {
    SongListQuery {
        search: Some(search.to_owned()),
        ..SongListQuery::default()
    }
}

fn album_query(search: &str) -> AlbumListQuery {
    AlbumListQuery {
        search: Some(search.to_owned()),
        ..AlbumListQuery::default()
    }
}

fn artist_query(search: &str) -> ArtistListQuery {
    ArtistListQuery {
        search: Some(search.to_owned()),
        ..ArtistListQuery::default()
    }
}

fn names(artists: &[ArtistRef]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn album_card(album: &Album) -> SearchCard {
    let artists = if album.album_artists.is_empty() {
        &album.artists
    } else {
        &album.album_artists
    };
    SearchCard {
        id: album.id.as_str().into(),
        title: album.name.as_str().into(),
        subtitle: names(artists).into(),
        ..SearchCard::default()
    }
}

fn artist_card(artist: &Artist) -> SearchCard {
    let subtitle = match artist.album_count {
        Some(1) => "1 album".to_owned(),
        Some(count) => format!("{count} albums"),
        None => String::new(),
    };
    SearchCard {
        id: artist.id.as_str().into(),
        title: artist.name.as_str().into(),
        subtitle: subtitle.into(),
        ..SearchCard::default()
    }
}

/// How a page of results landed in a [`Section`].
#[derive(Debug, PartialEq, Eq)]
enum Applied {
    Replaced,
    Appended(usize),
}

/// One result section. Items first come from `search`; "show more" then switches to the paged list
/// endpoint, refetching from the start once because both endpoints may order results differently.
#[derive(Debug)]
struct Section<T> {
    items: Vec<T>,
    offset: Option<usize>,
    has_more: bool,
    busy: bool,
}

impl<T> Default for Section<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            offset: None,
            has_more: false,
            busy: false,
        }
    }
}

impl<T> Section<T> {
    fn new(items: Vec<T>, limit: usize) -> Self {
        Self {
            has_more: items.len() >= limit,
            items,
            ..Self::default()
        }
    }

    fn next_page(&mut self) -> Option<Paging> {
        if self.busy || !self.has_more {
            return None;
        }
        self.busy = true;
        Some(match self.offset {
            None => Paging {
                start_index: 0,
                limit: self.items.len() + MORE_PAGE,
            },
            Some(start_index) => Paging {
                start_index,
                limit: MORE_PAGE,
            },
        })
    }

    fn apply(&mut self, paging: Paging, items: Vec<T>) -> Applied {
        self.busy = false;
        self.has_more = items.len() >= paging.limit;
        self.offset = Some(paging.start_index + items.len());
        if paging.start_index == 0 {
            self.items = items;
            Applied::Replaced
        } else {
            let start = self.items.len();
            self.items.extend(items);
            Applied::Appended(start)
        }
    }
}

enum More {
    Songs(Vec<Song>),
    Albums(Vec<Album>),
    Artists(Vec<Artist>),
}

/// Search results plus the bookkeeping that drops responses for superseded queries.
#[derive(Default)]
struct Searches {
    generation: u64,
    query: Option<String>,
    pending: bool,
    songs: Section<Song>,
    albums: Section<Album>,
    artists: Section<Artist>,
}

impl Searches {
    /// Starts a search for `query`, returning its generation, or `None` when nothing changed.
    fn begin(&mut self, query: Option<String>, force: bool) -> Option<u64> {
        if self.query == query && !force {
            return None;
        }
        self.generation += 1;
        self.pending = query.is_some();
        if query.is_none() {
            self.songs = Section::default();
            self.albums = Section::default();
            self.artists = Section::default();
        }
        self.query = query;
        Some(self.generation)
    }

    fn finish(&mut self, generation: u64, results: Option<SearchResults>, limit: usize) -> bool {
        if generation != self.generation {
            return false;
        }
        self.pending = false;
        if let Some(results) = results {
            self.songs = Section::new(results.songs, limit);
            self.albums = Section::new(results.albums, limit);
            self.artists = Section::new(results.artists, limit);
        }
        true
    }

    fn more(&mut self, kind: SearchKind) -> Option<(u64, String, Paging)> {
        if self.pending {
            return None;
        }
        let query = self.query.clone()?;
        let paging = match kind {
            SearchKind::Songs => self.songs.next_page(),
            SearchKind::Albums => self.albums.next_page(),
            SearchKind::Artists => self.artists.next_page(),
        }?;
        Some((self.generation, query, paging))
    }

    fn more_failed(&mut self, generation: u64, kind: SearchKind) {
        if generation == self.generation {
            match kind {
                SearchKind::Songs => self.songs.busy = false,
                SearchKind::Albums => self.albums.busy = false,
                SearchKind::Artists => self.artists.busy = false,
            }
        }
    }

    fn apply_more(&mut self, generation: u64, paging: Paging, more: More) -> Option<Applied> {
        (generation == self.generation).then(|| match more {
            More::Songs(items) => self.songs.apply(paging, items),
            More::Albums(items) => self.albums.apply(paging, items),
            More::Artists(items) => self.artists.apply(paging, items),
        })
    }

    fn song(&self, index: i32) -> Option<&Song> {
        self.songs.items.get(usize::try_from(index).ok()?)
    }
}

type CoverKey = (bool, String);

/// Bounded set of decoded covers currently held by the card models, oldest first.
#[derive(Default)]
struct Covers {
    loaded: VecDeque<CoverKey>,
    pending: HashSet<CoverKey>,
}

impl Covers {
    /// Whether the cover should be fetched; already loaded covers are just marked as recently used.
    fn wants(&mut self, key: CoverKey) -> bool {
        if let Some(position) = self.loaded.iter().position(|k| *k == key) {
            if let Some(key) = self.loaded.remove(position) {
                self.loaded.push_back(key);
            }
            return false;
        }
        self.pending.insert(key)
    }

    /// Records a loaded cover and returns the one to evict, if any.
    fn loaded(&mut self, key: CoverKey) -> Option<CoverKey> {
        self.pending.remove(&key);
        self.loaded.push_back(key);
        (self.loaded.len() > MAX_COVERS)
            .then(|| self.loaded.pop_front())
            .flatten()
    }

    fn failed(&mut self, key: &CoverKey) {
        self.pending.remove(key);
    }

    fn forget(&mut self, albums: bool) {
        self.loaded.retain(|(a, _)| *a != albums);
        self.pending.retain(|(a, _)| *a != albums);
    }
}

/// The song rows model. It also owns the rest of the search state so async completions can reach
/// it through the window.
#[derive(Default)]
struct Results {
    searches: RefCell<Searches>,
    covers: RefCell<Covers>,
    songs: VecModel<TrackRow>,
    albums: Rc<VecModel<SearchCard>>,
    artists: Rc<VecModel<SearchCard>>,
}

impl Model for Results {
    type Data = TrackRow;

    fn row_count(&self) -> usize {
        self.songs.row_count()
    }

    fn row_data(&self, row: usize) -> Option<TrackRow> {
        self.songs.row_data(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.songs.model_tracker()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Results {
    fn cards(&self, albums: bool) -> &VecModel<SearchCard> {
        if albums { &self.albums } else { &self.artists }
    }

    fn show_all(&self, window: &AppWindow) {
        for kind in [SearchKind::Songs, SearchKind::Albums, SearchKind::Artists] {
            self.show_section(kind, 0);
        }
        self.show_flags(window);
    }

    fn show_more(&self, window: &AppWindow, kind: SearchKind, applied: &Applied) {
        let start = match applied {
            Applied::Replaced => 0,
            Applied::Appended(start) => *start,
        };
        self.show_section(kind, start);
        self.show_flags(window);
    }

    /// Pushes the section's items from `start` on to its model, replacing it when `start` is 0.
    fn show_section(&self, kind: SearchKind, start: usize) {
        let searches = self.searches.borrow();
        if kind == SearchKind::Songs {
            let rows = searches.songs.items.iter().enumerate().skip(start);
            let rows = rows.map(|(i, song)| track_row(song, i));
            if start == 0 {
                self.songs.set_vec(rows.collect::<Vec<_>>());
            } else {
                self.songs.extend(rows);
            }
            return;
        }
        let albums = kind == SearchKind::Albums;
        let cards: Vec<SearchCard> = if albums {
            searches
                .albums
                .items
                .iter()
                .skip(start)
                .map(album_card)
                .collect()
        } else {
            searches
                .artists
                .items
                .iter()
                .skip(start)
                .map(artist_card)
                .collect()
        };
        if start == 0 {
            self.covers.borrow_mut().forget(albums);
            self.cards(albums).set_vec(cards);
        } else {
            self.cards(albums).extend(cards);
        }
    }

    fn show_flags(&self, window: &AppWindow) {
        let searches = self.searches.borrow();
        let state = window.global::<SearchState>();
        state.set_loading(searches.pending);
        state.set_songs_more(searches.songs.has_more);
        state.set_albums_more(searches.albums.has_more);
        state.set_artists_more(searches.artists.has_more);
        state.set_songs_busy(searches.songs.busy);
        state.set_albums_busy(searches.albums.busy);
        state.set_artists_busy(searches.artists.busy);
    }

    fn cover_id(&self, albums: bool, index: usize) -> Option<(String, String)> {
        let searches = self.searches.borrow();
        let (id, cover) = if albums {
            let album = searches.albums.items.get(index)?;
            (&album.id, &album.cover_art_id)
        } else {
            let artist = searches.artists.items.get(index)?;
            (&artist.id, &artist.cover_art_id)
        };
        Some((id.clone(), cover.clone()?))
    }

    fn set_cover(&self, albums: bool, index: usize, id: &str, cover: Option<slint::Image>) {
        let cards = self.cards(albums);
        let key = (albums, id.to_owned());
        let Some(mut card) = cards.row_data(index).filter(|card| card.id == id) else {
            self.covers.borrow_mut().failed(&key);
            return;
        };
        let Some(cover) = cover else {
            self.covers.borrow_mut().failed(&key);
            return;
        };
        card.cover = cover;
        card.has_cover = true;
        cards.set_row_data(index, card);
        let evicted = self.covers.borrow_mut().loaded(key);
        if let Some((albums, id)) = evicted {
            let cards = self.cards(albums);
            if let Some(index) = cards.iter().position(|card| card.id == id.as_str()) {
                cards.set_row_data(
                    index,
                    SearchCard {
                        cover: slint::Image::default(),
                        has_cover: false,
                        ..cards.row_data(index).unwrap_or_default()
                    },
                );
            }
        }
    }
}

fn with_results<R>(window: &AppWindow, f: impl FnOnce(&Results) -> R) -> Option<R> {
    let songs = window.global::<SearchState>().get_songs();
    songs.as_any().downcast_ref::<Results>().map(f)
}

/// Runs `request` against the connected server on the runtime and hands the result to `done` on the
/// UI thread. Returns `false` when no server is connected.
fn fetch<T, E, F>(
    window: &AppWindow,
    ctx: &Context,
    request: impl FnOnce(Arc<dyn MusicServer>) -> F + Send + 'static,
    done: impl FnOnce(&AppWindow, &Results, Result<T, E>) + Send + 'static,
) -> bool
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    let Some(server) = ctx.session.get() else {
        return false;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = request(server).await;
        on_ui(&weak, move |window| {
            with_results(window, |results| done(window, results, result));
        });
    });
    true
}

fn report(window: &AppWindow, what: &str, error: impl Display) {
    super::show_status(window, &format!("{what}: {error}"));
}

fn search(window: &AppWindow, ctx: &Context, query: Option<String>, force: bool) {
    let started = with_results(window, |results| {
        let generation = results.searches.borrow_mut().begin(query.clone(), force)?;
        if query.is_none() {
            results.show_all(window);
        } else {
            results.show_flags(window);
        }
        Some(generation)
    });
    let (Some(Some(generation)), Some(query)) = (started, query) else {
        return;
    };
    let sent = fetch(
        window,
        ctx,
        move |server| async move { server.search(&query, SEARCH_LIMIT).await },
        move |window, results, result| {
            let found = match result {
                Ok(found) => Some(found),
                Err(e) => {
                    report(window, "Search failed", e);
                    None
                }
            };
            let applied = results
                .searches
                .borrow_mut()
                .finish(generation, found, SEARCH_LIMIT);
            if applied {
                results.show_all(window);
            }
        },
    );
    if !sent {
        with_results(window, |results| {
            results
                .searches
                .borrow_mut()
                .finish(generation, None, SEARCH_LIMIT);
            results.show_flags(window);
        });
    }
}

fn show_more(window: &AppWindow, ctx: &Context, kind: SearchKind) {
    let Some(Some((generation, query, paging))) = with_results(window, |results| {
        let request = results.searches.borrow_mut().more(kind);
        results.show_flags(window);
        request
    }) else {
        return;
    };
    let request = move |server: Arc<dyn MusicServer>| async move {
        match kind {
            SearchKind::Songs => server
                .song_list(&song_query(&query), paging)
                .await
                .map(|page| More::Songs(page.items)),
            SearchKind::Albums => server
                .album_list(&album_query(&query), paging)
                .await
                .map(|page| More::Albums(page.items)),
            SearchKind::Artists => server
                .artist_list(&artist_query(&query), paging)
                .await
                .map(|page| More::Artists(page.items)),
        }
    };
    let sent = fetch(window, ctx, request, move |window, results, result| {
        let applied = match result {
            Ok(more) => results
                .searches
                .borrow_mut()
                .apply_more(generation, paging, more),
            Err(e) => {
                results.searches.borrow_mut().more_failed(generation, kind);
                report(window, "Could not load more results", e);
                None
            }
        };
        match applied {
            Some(applied) => results.show_more(window, kind, &applied),
            None => results.show_flags(window),
        }
    });
    if !sent {
        with_results(window, |results| {
            results.searches.borrow_mut().more_failed(generation, kind);
            results.show_flags(window);
        });
    }
}

fn load_cover(window: &AppWindow, ctx: &Context, kind: SearchKind, index: i32) {
    let albums = kind == SearchKind::Albums;
    let Ok(index) = usize::try_from(index) else {
        return;
    };
    let target = with_results(window, |results| {
        let (id, cover_id) = results.cover_id(albums, index)?;
        let wanted = results.covers.borrow_mut().wants((albums, id.clone()));
        wanted.then_some((id, cover_id))
    });
    let Some(Some((id, cover_id))) = target else {
        return;
    };
    let covers = Arc::clone(&ctx.covers);
    let key = (albums, id.clone());
    let sent = fetch(
        window,
        ctx,
        move |server| async move { covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await },
        move |_, results, result| {
            let image = result.ok().and_then(|cover| to_image(&cover));
            results.set_cover(albums, index, &id, image);
        },
    );
    if !sent {
        with_results(window, |results| {
            results.covers.borrow_mut().failed(&key);
        });
    }
}

fn song_command(
    window: &AppWindow,
    index: i32,
    command: impl FnOnce(&Searches, Song) -> Command,
) -> Option<Command> {
    with_results(window, |results| {
        let searches = results.searches.borrow();
        let song = searches.song(index)?.clone();
        Some(command(&searches, song))
    })
    .flatten()
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<SearchState>();
    let results = Rc::new(Results::default());
    state.set_albums(ModelRc::from(Rc::clone(&results.albums)));
    state.set_artists(ModelRc::from(Rc::clone(&results.artists)));
    state.set_songs(ModelRc::from(results));

    let debounce = Rc::new(Timer::default());

    let (weak, ctx_) = (window.as_weak(), ctx.clone());
    let timer = Rc::clone(&debounce);
    state.on_edited(move |text| {
        let query = normalize(&text);
        if query.is_none() {
            timer.stop();
            if let Some(window) = weak.upgrade() {
                search(&window, &ctx_, None, false);
            }
            return;
        }
        let (weak, ctx) = (weak.clone(), ctx_.clone());
        timer.start(TimerMode::SingleShot, DEBOUNCE, move || {
            if let Some(window) = weak.upgrade() {
                search(&window, &ctx, query.clone(), false);
            }
        });
    });

    let (weak, ctx_) = (window.as_weak(), ctx.clone());
    let timer = Rc::clone(&debounce);
    state.on_submit(move |text| {
        timer.stop();
        if let Some(window) = weak.upgrade() {
            search(&window, &ctx_, normalize(&text), true);
        }
    });

    let (weak, ctx_) = (window.as_weak(), ctx.clone());
    state.on_load(move |id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<SearchState>();
        if !id.is_empty() {
            state.set_query(id);
        }
        debounce.stop();
        search(&window, &ctx_, normalize(&state.get_query()), false);
    });

    let (weak, ctx_) = (window.as_weak(), ctx.clone());
    state.on_show_more(move |kind| {
        if let Some(window) = weak.upgrade() {
            show_more(&window, &ctx_, kind);
        }
    });

    let (weak, ctx_) = (window.as_weak(), ctx.clone());
    state.on_need_cover(move |kind, index| {
        if let Some(window) = weak.upgrade() {
            load_cover(&window, &ctx_, kind, index);
        }
    });

    let on_song = |command: fn(&Searches, Song) -> Command| {
        let (weak, ctx) = (window.as_weak(), ctx.clone());
        move |index: i32| {
            let command = weak
                .upgrade()
                .and_then(|window| song_command(&window, index, command));
            if let Some(command) = command {
                ctx.send(command);
            }
        }
    };
    state.on_play(on_song(|searches, song| Command::PlaySongs {
        songs: searches.songs.items.clone(),
        song_id: Some(song.id),
    }));
    state.on_play_only(on_song(|_, song| Command::PlaySongs {
        songs: vec![song],
        song_id: None,
    }));
    state.on_play_next(on_song(|_, song| Command::AddNext(vec![song])));
    state.on_add_to_queue(on_song(|_, song| Command::AddLast(vec![song])));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{FakeServer, test_song};

    fn songs(count: usize) -> Vec<Song> {
        (0..count)
            .map(|i| test_song(&format!("s{i}"), 1000))
            .collect()
    }

    #[test]
    fn blank_queries_are_ignored() {
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize("  river "), Some("river".to_owned()));
        assert_eq!(song_query("river").search.as_deref(), Some("river"));
        assert_eq!(album_query("river").search.as_deref(), Some("river"));
        assert_eq!(artist_query("river").search.as_deref(), Some("river"));
    }

    #[test]
    fn show_more_refetches_once_then_pages() {
        let mut section = Section::new(songs(SEARCH_LIMIT), SEARCH_LIMIT);
        assert!(section.has_more);
        let first = section.next_page();
        assert_eq!(
            first,
            Some(Paging {
                start_index: 0,
                limit: SEARCH_LIMIT + MORE_PAGE
            })
        );
        assert_eq!(section.next_page(), None, "busy sections do not refetch");

        let paging = first.unwrap_or(Paging {
            start_index: 0,
            limit: 0,
        });
        assert_eq!(
            section.apply(paging, songs(paging.limit)),
            Applied::Replaced
        );
        assert_eq!(section.items.len(), SEARCH_LIMIT + MORE_PAGE);

        let second = section.next_page();
        assert_eq!(
            second,
            Some(Paging {
                start_index: SEARCH_LIMIT + MORE_PAGE,
                limit: MORE_PAGE
            })
        );
        let paging = second.unwrap_or(paging);
        assert_eq!(
            section.apply(paging, songs(3)),
            Applied::Appended(SEARCH_LIMIT + MORE_PAGE)
        );
        assert!(!section.has_more);
        assert_eq!(section.next_page(), None);
    }

    #[test]
    fn short_results_have_no_more() {
        let section = Section::new(songs(3), SEARCH_LIMIT);
        assert!(!section.has_more);
    }

    #[test]
    fn stale_responses_are_dropped() {
        let mut searches = Searches::default();
        let first = searches.begin(Some("riv".into()), false);
        let second = searches.begin(Some("river".into()), false);
        assert_eq!(searches.begin(Some("river".into()), false), None);
        assert!(searches.pending);
        assert_eq!(searches.more(SearchKind::Songs), None);

        let results = |n| SearchResults {
            songs: songs(n),
            albums: Vec::new(),
            artists: Vec::new(),
        };
        assert!(!searches.finish(first.unwrap_or_default(), Some(results(1)), SEARCH_LIMIT));
        assert!(searches.songs.items.is_empty());
        assert!(searches.finish(
            second.unwrap_or_default(),
            Some(results(SEARCH_LIMIT)),
            SEARCH_LIMIT
        ));
        assert!(!searches.pending);
        assert_eq!(searches.song(0).map(|s| s.id.as_str()), Some("s0"));

        let (generation, query, paging) = searches.more(SearchKind::Songs).unwrap_or((
            0,
            String::new(),
            Paging {
                start_index: 0,
                limit: 0,
            },
        ));
        assert_eq!(query, "river");
        searches.begin(Some("rivers".into()), false);
        assert_eq!(
            searches.apply_more(generation, paging, More::Songs(songs(5))),
            None
        );

        searches.begin(None, false);
        assert!(!searches.pending);
        assert!(searches.songs.items.is_empty());
    }

    #[test]
    fn covers_are_bounded_and_deduplicated() {
        let mut covers = Covers::default();
        let key = |i: usize| (true, format!("album-{i}"));
        assert!(covers.wants(key(0)));
        assert!(!covers.wants(key(0)), "in flight");
        for i in 0..MAX_COVERS {
            assert_eq!(covers.loaded(key(i)), None);
        }
        assert!(!covers.wants(key(0)), "already loaded, now most recent");
        assert_eq!(covers.loaded(key(MAX_COVERS)), Some(key(1)));
        covers.forget(true);
        assert!(covers.wants(key(0)));
    }

    #[tokio::test]
    async fn demo_results_map_to_cards() {
        let server = FakeServer::new(1000);
        let found = server.search("ada", SEARCH_LIMIT).await.ok();
        let found = found.unwrap_or(SearchResults {
            songs: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        });
        let album = found.albums.first().map(album_card).unwrap_or_default();
        assert!(album.id.starts_with("album-"));
        assert!(album.subtitle.contains("Ada"));
        let artist = found.artists.first().map(artist_card).unwrap_or_default();
        assert!(artist.title.starts_with("Ada"));
    }
}
