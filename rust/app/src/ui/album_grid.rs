//! Lazy album cover grid: pages albums on demand, groups them into rows of `columns` cards and
//! loads covers only for the rows Slint asks for, keeping a bounded number of decoded images.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use feishin_api::MusicServer;
use feishin_core::model::{Album, AlbumListQuery, Paging, Song};
use slint::{
    ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, SharedString, VecModel,
};

use super::{Actions, AlbumGridCard, AlbumGridRow, AppWindow, Context, on_ui, show_status};
use crate::controller::Command;

pub const PAGE_SIZE: usize = 60;
pub const COVER_SIZE: u32 = 256;
const COVER_CAPACITY: usize = 96;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardData {
    pub id: SharedString,
    pub name: SharedString,
    pub artists: SharedString,
    pub artist_id: SharedString,
    pub year: SharedString,
    pub cover_art_id: Option<String>,
}

impl CardData {
    pub fn from_album(album: &Album) -> Self {
        let artists = if album.album_artists.is_empty() {
            &album.artists
        } else {
            &album.album_artists
        };
        Self {
            id: album.id.as_str().into(),
            name: album.name.as_str().into(),
            artists: artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
                .into(),
            artist_id: artists
                .first()
                .map(|artist| artist.id.as_str())
                .unwrap_or_default()
                .into(),
            year: album.year.map(|y| y.to_string()).unwrap_or_default().into(),
            cover_art_id: album.cover_art_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GridRequest {
    Count {
        generation: u64,
        query: AlbumListQuery,
    },
    Page {
        generation: u64,
        query: AlbumListQuery,
        page: usize,
    },
    Cover {
        cover_art_id: String,
    },
}

/// Least recently used images keyed by cover art id, plus the ids already requested or failed.
pub struct CoverLru<T> {
    capacity: usize,
    entries: VecDeque<(String, T)>,
    pending: HashSet<String>,
    failed: HashSet<String>,
}

impl<T: Clone> CoverLru<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    pub fn get(&mut self, id: &str) -> Option<T> {
        let position = self.entries.iter().position(|(key, _)| key == id)?;
        let entry = self.entries.remove(position)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    /// Returns true, and marks `id` as requested, when it is not cached, in flight or failed.
    pub fn begin(&mut self, id: &str) -> bool {
        if self.failed.contains(id)
            || self.pending.contains(id)
            || self.entries.iter().any(|(key, _)| key == id)
        {
            return false;
        }
        self.pending.insert(id.to_owned());
        true
    }

    pub fn finish(&mut self, id: &str, value: Option<T>) {
        self.pending.remove(id);
        let Some(value) = value else {
            self.failed.insert(id.to_owned());
            return;
        };
        self.entries.retain(|(key, _)| key != id);
        self.entries.push_back((id.to_owned(), value));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.pending.clear();
        self.failed.clear();
    }
}

struct GridState {
    generation: u64,
    query: Option<AlbumListQuery>,
    total: usize,
    columns: usize,
    pages: HashMap<usize, Vec<CardData>>,
    requested: HashSet<usize>,
    covers: CoverLru<Image>,
}

impl GridState {
    fn card(&self, index: usize) -> Option<&CardData> {
        self.pages.get(&(index / PAGE_SIZE))?.get(index % PAGE_SIZE)
    }

    fn rows_for(&self, indexes: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
        indexes
            .into_iter()
            .map(|index| index / self.columns)
            .collect()
    }
}

/// Rows of album cards for `AlbumGrid`, paged from the server.
pub struct AlbumGridModel {
    state: RefCell<GridState>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(GridRequest)>,
}

impl AlbumGridModel {
    pub fn new(fetch: impl Fn(GridRequest) + 'static) -> Self {
        Self {
            state: RefCell::new(GridState {
                generation: 0,
                query: None,
                total: 0,
                columns: 1,
                pages: HashMap::new(),
                requested: HashSet::new(),
                covers: CoverLru::new(COVER_CAPACITY),
            }),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    /// Starts paging `query` from the server.
    pub fn reset(&self, query: AlbumListQuery) {
        let generation = {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            state.query = Some(query.clone());
            state.total = 0;
            state.pages.clear();
            state.requested.clear();
            state.generation
        };
        self.notify.reset();
        (self.fetch)(GridRequest::Count { generation, query });
    }

    pub fn set_columns(&self, columns: usize) {
        {
            let mut state = self.state.borrow_mut();
            let columns = columns.max(1);
            if state.columns == columns {
                return;
            }
            state.columns = columns;
        }
        self.notify.reset();
    }

    pub fn clear_covers(&self) {
        self.state.borrow_mut().covers.clear();
    }

    pub fn apply_count(&self, generation: u64, total: usize) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            state.total = total;
        }
        self.notify.reset();
        true
    }

    pub fn apply_page(&self, generation: u64, page: usize, cards: Vec<CardData>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = page * PAGE_SIZE;
            let end = (start + cards.len()).min(state.total);
            state.pages.insert(page, cards);
            state.rows_for(start..end)
        };
        for row in rows {
            self.notify.row_changed(row);
        }
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        let mut state = self.state.borrow_mut();
        if state.generation == generation {
            state.requested.remove(&page);
        }
    }

    pub fn apply_cover(&self, cover_art_id: &str, image: Option<Image>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            let loaded = image.is_some();
            state.covers.finish(cover_art_id, image);
            if !loaded {
                return;
            }
            let indexes: Vec<usize> = state
                .pages
                .iter()
                .flat_map(|(page, cards)| {
                    cards
                        .iter()
                        .enumerate()
                        .filter(|(_, card)| card.cover_art_id.as_deref() == Some(cover_art_id))
                        .map(move |(i, _)| page * PAGE_SIZE + i)
                })
                .collect();
            state.rows_for(indexes)
        };
        for row in rows {
            self.notify.row_changed(row);
        }
    }
}

impl Model for AlbumGridModel {
    type Data = AlbumGridRow;

    fn row_count(&self) -> usize {
        let state = self.state.borrow();
        state.total.div_ceil(state.columns)
    }

    fn row_data(&self, row: usize) -> Option<AlbumGridRow> {
        let mut requests = Vec::new();
        let cards = {
            let mut state = self.state.borrow_mut();
            let start = row * state.columns;
            if start >= state.total {
                return None;
            }
            let end = (start + state.columns).min(state.total);
            let mut cards = Vec::with_capacity(end - start);
            for index in start..end {
                let Some(data) = state.card(index).cloned() else {
                    let page = index / PAGE_SIZE;
                    if let Some(query) = state.query.clone()
                        && state.requested.insert(page)
                    {
                        requests.push(GridRequest::Page {
                            generation: state.generation,
                            query,
                            page,
                        });
                    }
                    cards.push(AlbumGridCard::default());
                    continue;
                };
                let cover = data.cover_art_id.as_deref().and_then(|id| {
                    let cached = state.covers.get(id);
                    if cached.is_none() && state.covers.begin(id) {
                        requests.push(GridRequest::Cover {
                            cover_art_id: id.to_owned(),
                        });
                    }
                    cached
                });
                cards.push(AlbumGridCard {
                    id: data.id,
                    name: data.name,
                    artists: data.artists,
                    artist_id: data.artist_id,
                    year: data.year,
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    loaded: true,
                });
            }
            cards
        };
        for request in requests {
            (self.fetch)(request);
        }
        Some(AlbumGridRow {
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Finds the grid model a view keeps in its global.
pub type Locate = fn(&AppWindow) -> ModelRc<AlbumGridRow>;

pub fn with_model<R>(
    window: &AppWindow,
    locate: Locate,
    f: impl FnOnce(&AlbumGridModel) -> R,
) -> Option<R> {
    let rows = locate(window);
    rows.as_any().downcast_ref::<AlbumGridModel>().map(f)
}

/// Runs a grid request on the I/O runtime and applies its result to the model `locate` finds.
/// `on_count` runs on the UI thread after a count for the current generation is applied.
pub fn spawn_request(
    window: slint::Weak<AppWindow>,
    ctx: &Context,
    locate: Locate,
    on_count: fn(&AppWindow, anyhow::Result<usize>),
    request: GridRequest,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        match request {
            GridRequest::Count { generation, query } => {
                let result = server.album_count(&query).await;
                on_ui(&window, move |window| {
                    let total = result.as_ref().map_or(0, |total| *total);
                    if with_model(window, locate, |model| model.apply_count(generation, total))
                        == Some(true)
                    {
                        on_count(window, result.map_err(Into::into));
                    }
                });
            }
            GridRequest::Page {
                generation,
                query,
                page,
            } => {
                let paging = Paging {
                    start_index: page * PAGE_SIZE,
                    limit: PAGE_SIZE,
                };
                let result = server
                    .album_list(&query, paging)
                    .await
                    .map(|albums| albums.items.iter().map(CardData::from_album).collect());
                on_ui(&window, move |window| match result {
                    Ok(cards) => {
                        with_model(window, locate, |model| {
                            model.apply_page(generation, page, cards)
                        });
                    }
                    Err(e) => {
                        with_model(window, locate, |model| model.page_failed(generation, page));
                        show_status(window, &format!("Could not load albums: {e}"));
                    }
                });
            }
            GridRequest::Cover { cover_art_id } => {
                let result = covers
                    .cover(server.as_ref(), &cover_art_id, COVER_SIZE)
                    .await;
                if let Err(e) = &result {
                    tracing::debug!("cover {cover_art_id}: {e:#}");
                }
                on_ui(&window, move |window| {
                    let image = result
                        .ok()
                        .and_then(|cover| super::player::to_image(&cover));
                    with_model(window, locate, |model| {
                        model.apply_cover(&cover_art_id, image)
                    });
                });
            }
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumAction {
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
}

async fn album_songs(server: &dyn MusicServer, album_id: &str) -> anyhow::Result<Vec<Song>> {
    Ok(server.album(album_id).await?.songs)
}

/// Fetches an album's songs and plays, queues or offers them for a playlist.
pub fn album_action(window: &AppWindow, ctx: &Context, album_id: String, action: AlbumAction) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ctx.rt.clone().spawn(async move {
        let songs = match album_songs(server.as_ref(), &album_id).await {
            Ok(songs) => songs,
            Err(e) => {
                on_ui(&weak, move |window| {
                    show_status(window, &format!("Could not load the album: {e:#}"));
                });
                return;
            }
        };
        match action {
            AlbumAction::Play => ctx.send(Command::PlaySongs {
                songs,
                song_id: None,
            }),
            AlbumAction::PlayNext => ctx.send(Command::AddNext(songs)),
            AlbumAction::AddToQueue => ctx.send(Command::AddLast(songs)),
            AlbumAction::AddToPlaylist => on_ui(&weak, move |window| {
                let ids: Vec<SharedString> =
                    songs.iter().map(|song| song.id.as_str().into()).collect();
                window
                    .global::<Actions>()
                    .invoke_add_to_playlist(ModelRc::new(VecModel::from(ids)));
            }),
        }
    });
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::demo::FakeServer;

    fn album(index: usize) -> Album {
        Album {
            id: format!("album-{index}"),
            server_id: "demo".into(),
            name: format!("Album {index}"),
            album_artists: Vec::new(),
            artists: vec![feishin_core::model::ArtistRef {
                id: "artist-1".into(),
                name: "Ada Vale".into(),
            }],
            year: Some(1999),
            genres: Vec::new(),
            song_count: 10,
            duration_ms: 0,
            cover_art_id: Some(format!("al-{index}")),
            play_count: 0,
            user_rating: None,
            favorite: false,
            created_at: None,
            last_played_at: None,
        }
    }

    fn recording() -> (Rc<RefCell<Vec<GridRequest>>>, AlbumGridModel) {
        let log: Rc<RefCell<Vec<GridRequest>>> = Rc::default();
        let sink = Rc::clone(&log);
        let model = AlbumGridModel::new(move |request| sink.borrow_mut().push(request));
        (log, model)
    }

    #[test]
    fn lru_evicts_oldest_and_tracks_requests() {
        let mut lru = CoverLru::new(2);
        assert!(lru.begin("a"));
        assert!(!lru.begin("a"));
        lru.finish("a", Some(1));
        lru.finish("b", Some(2));
        assert_eq!(lru.get("a"), Some(1));
        lru.finish("c", Some(3));
        assert_eq!(lru.get("b"), None);
        assert_eq!(lru.get("a"), Some(1));
        assert!(lru.begin("d"));
        lru.finish("d", None);
        assert!(!lru.begin("d"));
        lru.clear();
        assert!(lru.begin("d"));
    }

    #[test]
    fn cards_prefer_album_artists_and_fall_back_to_artists() {
        let card = CardData::from_album(&album(3));
        assert_eq!(card.artists, "Ada Vale");
        assert_eq!(card.artist_id, "artist-1");
        assert_eq!(card.year, "1999");
        assert_eq!(card.cover_art_id.as_deref(), Some("al-3"));
    }

    #[test]
    fn rows_regroup_by_columns_and_request_pages_and_covers_once() {
        let (log, model) = recording();
        model.set_columns(4);
        model.reset(AlbumListQuery::default());
        assert!(matches!(
            log.borrow_mut().drain(..).collect::<Vec<_>>().as_slice(),
            [GridRequest::Count { generation: 1, .. }]
        ));
        assert!(model.apply_count(1, 130));
        assert_eq!(model.row_count(), 33);

        let row = model.row_data(15).expect("row");
        assert_eq!(row.cards.row_count(), 4);
        assert!(row.cards.iter().all(|card| !card.loaded));
        model.row_data(16);
        let requests: Vec<_> = log.borrow_mut().drain(..).collect();
        assert!(matches!(
            requests.as_slice(),
            [GridRequest::Page {
                generation: 1,
                page: 1,
                ..
            }]
        ));

        let cards = (60..120).map(|i| CardData::from_album(&album(i))).collect();
        model.apply_page(1, 1, cards);
        let row = model.row_data(15).expect("row");
        assert_eq!(row.cards.row_data(0).map(|c| c.id), Some("album-60".into()));
        assert!(row.cards.iter().all(|card| card.loaded && !card.has_cover));
        let covers = log
            .borrow_mut()
            .drain(..)
            .filter(|r| matches!(r, GridRequest::Cover { .. }))
            .count();
        assert_eq!(covers, 4);
        model.row_data(15);
        assert!(log.borrow().is_empty());

        model.apply_cover("al-61", Some(Image::default()));
        let row = model.row_data(15).expect("row");
        assert_eq!(row.cards.row_data(1).map(|c| c.has_cover), Some(true));

        model.set_columns(10);
        assert_eq!(model.row_count(), 13);
        assert_eq!(model.row_data(13), None);

        model.reset(AlbumListQuery::default());
        model.apply_page(1, 0, vec![CardData::default()]);
        assert!(!model.apply_count(1, 5));
        assert_eq!(model.row_count(), 0);
    }

    #[tokio::test]
    async fn album_songs_come_from_the_server() {
        let server = FakeServer::new(48);
        let songs = album_songs(&server, "album-1").await.unwrap_or_default();
        assert_eq!(songs.len(), 12);
        assert_eq!(songs[0].album_id.as_deref(), Some("album-1"));
    }
}
