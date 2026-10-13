use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use feishin_core::model::{Album, AlbumListQuery, AlbumSort, Paging, SortOrder};
use slint::{
    ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, SharedString, VecModel,
};

use super::player::to_image;
use super::{AppWindow, Context, HomeAlbum, HomeShelf, HomeState, on_ui, show_status};
use crate::controller::Command;

pub const SHELF_SIZE: usize = 20;
const COVER_SIZE: u32 = 192;
const MAX_COVERS: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Random,
}

impl Shelf {
    pub const ALL: [Self; 4] = [
        Self::RecentlyAdded,
        Self::RecentlyPlayed,
        Self::MostPlayed,
        Self::Random,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::RecentlyAdded => "Recently added",
            Self::RecentlyPlayed => "Recently played",
            Self::MostPlayed => "Most played",
            Self::Random => "Random",
        }
    }

    pub fn query(self) -> AlbumListQuery {
        let (sort, order) = match self {
            Self::RecentlyAdded => (AlbumSort::RecentlyAdded, SortOrder::Desc),
            Self::RecentlyPlayed => (AlbumSort::RecentlyPlayed, SortOrder::Desc),
            Self::MostPlayed => (AlbumSort::PlayCount, SortOrder::Desc),
            Self::Random => (AlbumSort::Random, SortOrder::Asc),
        };
        AlbumListQuery {
            sort,
            order,
            ..AlbumListQuery::default()
        }
    }

    /// Random is only fetched again on demand so it stays put while browsing.
    fn reload_on_visit(self) -> bool {
        self != Self::Random
    }
}

pub fn album_card(album: &Album) -> HomeAlbum {
    let artists = if album.album_artists.is_empty() {
        &album.artists
    } else {
        &album.album_artists
    };
    HomeAlbum {
        id: album.id.as_str().into(),
        name: album.name.as_str().into(),
        artist: artists
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
        cover_id: album.cover_art_id.as_deref().unwrap_or_default().into(),
        cover: Image::default(),
        has_cover: false,
    }
}

/// Decoded covers currently shown, oldest first, so their number stays bounded.
#[derive(Default)]
struct CoverLru {
    entries: VecDeque<(usize, SharedString)>,
}

impl CoverLru {
    /// Records a shown cover and returns the ones to drop to stay within `capacity`.
    fn insert(
        &mut self,
        shelf: usize,
        id: SharedString,
        capacity: usize,
    ) -> Vec<(usize, SharedString)> {
        self.entries.retain(|(s, i)| (*s, i) != (shelf, &id));
        self.entries.push_back((shelf, id));
        let excess = self.entries.len().saturating_sub(capacity);
        self.entries.drain(..excess).collect()
    }

    fn forget_shelf(&mut self, shelf: usize) {
        self.entries.retain(|(s, _)| *s != shelf);
    }
}

struct Slot {
    kind: Shelf,
    albums: Rc<VecModel<HomeAlbum>>,
    generation: Cell<u64>,
    loading: Cell<bool>,
    pending: RefCell<HashSet<SharedString>>,
}

/// The home shelves: one album row per [`Shelf`], with stale-response guards and bounded covers.
pub struct HomeModel {
    slots: Vec<Slot>,
    covers: RefCell<CoverLru>,
    server: RefCell<Option<String>>,
    notify: ModelNotify,
}

impl Default for HomeModel {
    fn default() -> Self {
        Self {
            slots: Shelf::ALL
                .iter()
                .map(|&kind| Slot {
                    kind,
                    albums: Rc::default(),
                    generation: Cell::default(),
                    loading: Cell::default(),
                    pending: RefCell::default(),
                })
                .collect(),
            covers: RefCell::default(),
            server: RefCell::default(),
            notify: ModelNotify::default(),
        }
    }
}

impl HomeModel {
    fn set_loading(&self, shelf: usize, loading: bool) {
        if self.slots[shelf].loading.replace(loading) != loading {
            self.notify.row_changed(shelf);
        }
    }

    /// Drops everything when the connected server changed. Returns the shelves to fetch.
    pub fn visit(&self, server_id: &str) -> Vec<usize> {
        let switched = self.server.borrow().as_deref() != Some(server_id);
        if switched {
            *self.server.borrow_mut() = Some(server_id.to_owned());
            self.covers.borrow_mut().entries.clear();
            for (shelf, slot) in self.slots.iter().enumerate() {
                slot.generation.set(slot.generation.get() + 1);
                slot.pending.borrow_mut().clear();
                slot.albums.clear();
                self.set_loading(shelf, false);
            }
        }
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.kind.reload_on_visit() || slot.albums.row_count() == 0)
            .map(|(shelf, _)| shelf)
            .collect()
    }

    /// Starts a fetch for `shelf`, invalidating any in flight. Returns its generation and query.
    pub fn begin(&self, shelf: usize) -> Option<(u64, AlbumListQuery)> {
        let slot = self.slots.get(shelf)?;
        slot.generation.set(slot.generation.get() + 1);
        if slot.albums.row_count() == 0 {
            self.set_loading(shelf, true);
        }
        Some((slot.generation.get(), slot.kind.query()))
    }

    /// Applies a fetched page unless a newer fetch started. Unchanged rows keep their covers.
    pub fn apply(&self, shelf: usize, generation: u64, albums: &[Album]) -> bool {
        let Some(slot) = self.slots.get(shelf) else {
            return false;
        };
        if slot.generation.get() != generation {
            return false;
        }
        self.set_loading(shelf, false);
        let same = slot.albums.row_count() == albums.len()
            && slot
                .albums
                .iter()
                .zip(albums)
                .all(|(card, album)| card.id == album.id.as_str());
        if !same {
            self.covers.borrow_mut().forget_shelf(shelf);
            slot.pending.borrow_mut().clear();
            slot.albums
                .set_vec(albums.iter().map(album_card).collect::<Vec<_>>());
        }
        true
    }

    pub fn failed(&self, shelf: usize, generation: u64) -> bool {
        let current = self
            .slots
            .get(shelf)
            .is_some_and(|slot| slot.generation.get() == generation);
        if current {
            self.set_loading(shelf, false);
        }
        current
    }

    /// Marks the cover of a card as requested. Returns `(album id, cover art id)` if it must be fetched.
    pub fn want_cover(&self, shelf: usize, index: usize) -> Option<(SharedString, String)> {
        let slot = self.slots.get(shelf)?;
        let card = slot.albums.row_data(index)?;
        if card.has_cover || card.cover_id.is_empty() {
            return None;
        }
        slot.pending
            .borrow_mut()
            .insert(card.id.clone())
            .then(|| (card.id, card.cover_id.to_string()))
    }

    /// Shows a fetched cover (`None` on failure) and releases the oldest ones beyond the bound.
    pub fn set_cover(&self, shelf: usize, id: &SharedString, cover: Option<Image>) {
        let Some(slot) = self.slots.get(shelf) else {
            return;
        };
        if !slot.pending.borrow_mut().remove(id) {
            return;
        }
        let Some(cover) = cover else {
            return;
        };
        if !self.update_card(shelf, id, |card| {
            card.cover = cover;
            card.has_cover = true;
        }) {
            return;
        }
        let evicted = self
            .covers
            .borrow_mut()
            .insert(shelf, id.clone(), MAX_COVERS);
        for (shelf, id) in evicted {
            self.update_card(shelf, &id, |card| {
                card.cover = Image::default();
                card.has_cover = false;
            });
        }
    }

    fn update_card(&self, shelf: usize, id: &SharedString, f: impl FnOnce(&mut HomeAlbum)) -> bool {
        let albums = &self.slots[shelf].albums;
        let Some(index) = albums.iter().position(|card| card.id == *id) else {
            return false;
        };
        if let Some(mut card) = albums.row_data(index) {
            f(&mut card);
            albums.set_row_data(index, card);
        }
        true
    }

    #[cfg(test)]
    fn cards(&self, shelf: usize) -> Vec<HomeAlbum> {
        self.slots[shelf].albums.iter().collect()
    }
}

impl Model for HomeModel {
    type Data = HomeShelf;

    fn row_count(&self) -> usize {
        self.slots.len()
    }

    fn row_data(&self, row: usize) -> Option<HomeShelf> {
        let slot = self.slots.get(row)?;
        Some(HomeShelf {
            title: slot.kind.title().into(),
            albums: ModelRc::from(Rc::clone(&slot.albums)),
            loading: slot.loading.get(),
            refreshable: slot.kind == Shelf::Random,
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn with_model<R>(window: &AppWindow, f: impl FnOnce(&HomeModel) -> R) -> Option<R> {
    let shelves = window.global::<HomeState>().get_shelves();
    shelves.as_any().downcast_ref::<HomeModel>().map(f)
}

fn load_shelf(window: &AppWindow, ctx: &Context, shelf: usize) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(Some((generation, query))) = with_model(window, |model| model.begin(shelf)) else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let paging = Paging {
            start_index: 0,
            limit: SHELF_SIZE,
        };
        let result = server.album_list(&query, paging).await;
        on_ui(&weak, move |window| match result {
            Ok(page) => {
                with_model(window, |model| model.apply(shelf, generation, &page.items));
            }
            Err(e) => {
                if with_model(window, |model| model.failed(shelf, generation)) == Some(true) {
                    show_status(window, &format!("Could not load albums: {e}"));
                }
            }
        });
    });
}

fn load_cover(window: &AppWindow, ctx: &Context, shelf: usize, index: usize) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(Some((id, cover_id))) = with_model(window, |model| model.want_cover(shelf, index))
    else {
        return;
    };
    let covers = Arc::clone(&ctx.covers);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await;
        if let Err(e) = &result {
            tracing::debug!("cover {cover_id}: {e:#}");
        }
        on_ui(&weak, move |window| {
            let image = result.ok().and_then(|cover| to_image(&cover));
            with_model(window, |model| model.set_cover(shelf, &id, image));
        });
    });
}

fn play_album(window: &AppWindow, ctx: &Context, latest: &Arc<AtomicU64>, album_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let request = latest.fetch_add(1, Ordering::Relaxed) + 1;
    let latest = Arc::clone(latest);
    let ctx = ctx.clone();
    let weak = window.as_weak();
    ctx.rt.clone().spawn(async move {
        match server.album(&album_id).await {
            Ok(detail) if latest.load(Ordering::Relaxed) == request => {
                ctx.send(Command::PlaySongs {
                    songs: detail.songs,
                    song_id: None,
                });
            }
            Ok(_) => {}
            Err(e) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not play album: {e}"));
            }),
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<HomeState>();
    state.set_shelves(ModelRc::new(HomeModel::default()));

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    state.on_load(move |_| {
        let (Some(window), Some(server)) = (weak.upgrade(), load_ctx.session.get()) else {
            return;
        };
        let shelves = with_model(&window, |model| model.visit(&server.config().id));
        for shelf in shelves.unwrap_or_default() {
            load_shelf(&window, &load_ctx, shelf);
        }
    });

    let weak = window.as_weak();
    let refresh_ctx = ctx.clone();
    state.on_refresh(move |shelf| {
        if let (Some(window), Ok(shelf)) = (weak.upgrade(), usize::try_from(shelf)) {
            load_shelf(&window, &refresh_ctx, shelf);
        }
    });

    let weak = window.as_weak();
    let cover_ctx = ctx.clone();
    state.on_cover_needed(move |shelf, index| {
        if let (Some(window), Ok(shelf), Ok(index)) = (
            weak.upgrade(),
            usize::try_from(shelf),
            usize::try_from(index),
        ) {
            load_cover(&window, &cover_ctx, shelf, index);
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    let latest = Arc::new(AtomicU64::new(0));
    state.on_play(move |album_id| {
        if let Some(window) = weak.upgrade() {
            play_album(&window, &play_ctx, &latest, album_id.to_string());
        }
    });
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ArtistRef;

    use super::*;

    fn album(id: &str) -> Album {
        Album {
            id: id.to_owned(),
            server_id: "demo".into(),
            name: format!("Name {id}"),
            album_artists: vec![ArtistRef {
                id: "artist-1".into(),
                name: "Ada Vale".into(),
            }],
            artists: Vec::new(),
            year: None,
            genres: Vec::new(),
            song_count: 1,
            duration_ms: 0,
            cover_art_id: Some(format!("al-{id}")),
            play_count: 0,
            user_rating: None,
            favorite: false,
            created_at: None,
            last_played_at: None,
        }
    }

    fn albums(ids: &[&str]) -> Vec<Album> {
        ids.iter().map(|id| album(id)).collect()
    }

    fn red() -> Image {
        Image::from_rgba8(slint::SharedPixelBuffer::new(1, 1))
    }

    #[test]
    fn shelves_map_to_feishin_album_queries() {
        let sorts: Vec<_> = Shelf::ALL
            .iter()
            .map(|shelf| (shelf.query().sort, shelf.query().order))
            .collect();
        assert_eq!(
            sorts,
            [
                (AlbumSort::RecentlyAdded, SortOrder::Desc),
                (AlbumSort::RecentlyPlayed, SortOrder::Desc),
                (AlbumSort::PlayCount, SortOrder::Desc),
                (AlbumSort::Random, SortOrder::Asc),
            ]
        );
    }

    #[test]
    fn cards_show_album_artists_and_fall_back_to_track_artists() {
        let card = album_card(&album("a"));
        assert_eq!(
            (
                card.name.as_str(),
                card.artist.as_str(),
                card.artist_id.as_str()
            ),
            ("Name a", "Ada Vale", "artist-1")
        );
        assert_eq!(card.cover_id, "al-a");

        let mut only_artists = album("b");
        only_artists.artists = std::mem::take(&mut only_artists.album_artists);
        assert_eq!(album_card(&only_artists).artist, "Ada Vale");
    }

    #[test]
    fn stale_responses_are_ignored() {
        let model = HomeModel::default();
        assert_eq!(model.visit("s1"), [0, 1, 2, 3]);
        let first = model.begin(0).map(|(generation, _)| generation);
        let second = model.begin(0).map(|(generation, _)| generation);
        assert!(model.row_data(0).is_some_and(|shelf| shelf.loading));

        assert!(!model.apply(0, first.unwrap_or_default(), &albums(&["old"])));
        assert!(model.cards(0).is_empty());
        assert!(model.apply(0, second.unwrap_or_default(), &albums(&["a", "b"])));
        assert!(model.row_data(0).is_some_and(|shelf| !shelf.loading));
        assert_eq!(model.cards(0).len(), 2);
    }

    #[test]
    fn random_is_kept_between_visits_until_the_server_changes() {
        let model = HomeModel::default();
        model.visit("s1");
        for shelf in 0..4 {
            let generation = model.begin(shelf).map(|(g, _)| g).unwrap_or_default();
            model.apply(shelf, generation, &albums(&["a"]));
        }
        assert_eq!(model.visit("s1"), [0, 1, 2]);
        assert_eq!(model.visit("s2"), [0, 1, 2, 3]);
        assert!(model.cards(3).is_empty());
    }

    #[test]
    fn covers_are_requested_once_and_kept_when_rows_do_not_change() {
        let model = HomeModel::default();
        model.visit("s1");
        let generation = model.begin(0).map(|(g, _)| g).unwrap_or_default();
        model.apply(0, generation, &albums(&["a", "b"]));

        let wanted = model.want_cover(0, 1);
        assert_eq!(wanted, Some(("b".into(), "al-b".into())));
        assert_eq!(model.want_cover(0, 1), None);
        model.set_cover(0, &"b".into(), Some(red()));
        assert!(model.cards(0)[1].has_cover);
        assert_eq!(model.want_cover(0, 1), None);

        let generation = model.begin(0).map(|(g, _)| g).unwrap_or_default();
        model.apply(0, generation, &albums(&["a", "b"]));
        assert!(model.cards(0)[1].has_cover);

        let generation = model.begin(0).map(|(g, _)| g).unwrap_or_default();
        model.apply(0, generation, &albums(&["c", "b"]));
        assert!(!model.cards(0)[1].has_cover);
    }

    #[test]
    fn decoded_covers_stay_bounded() {
        let model = HomeModel::default();
        model.visit("s1");
        let ids: Vec<String> = (0..MAX_COVERS + 5).map(|i| format!("a{i}")).collect();
        let list: Vec<Album> = ids.iter().map(|id| album(id)).collect();
        let generation = model.begin(1).map(|(g, _)| g).unwrap_or_default();
        model.apply(1, generation, &list);
        for index in 0..list.len() {
            if let Some((id, _)) = model.want_cover(1, index) {
                model.set_cover(1, &id, Some(red()));
            }
        }
        let cards = model.cards(1);
        assert_eq!(cards.iter().filter(|c| c.has_cover).count(), MAX_COVERS);
        assert!(!cards[0].has_cover && cards[list.len() - 1].has_cover);
        assert!(model.want_cover(1, 0).is_some());
    }

    #[test]
    fn failed_covers_can_be_requested_again() {
        let model = HomeModel::default();
        model.visit("s1");
        let generation = model.begin(2).map(|(g, _)| g).unwrap_or_default();
        model.apply(2, generation, &albums(&["a"]));
        let wanted = model.want_cover(2, 0).map(|(id, _)| id);
        model.set_cover(2, &wanted.unwrap_or_default(), None);
        assert!(!model.cards(2)[0].has_cover);
        assert!(model.want_cover(2, 0).is_some());
    }

    mod headless {
        use std::sync::Mutex;
        use std::time::{Duration, Instant};

        use feishin_api::MusicServer;
        use slint::platform::software_renderer::{
            MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
        };
        use slint::platform::{EventLoopProxy, Platform, WindowAdapter};
        use slint::{EventLoopError, PhysicalSize, PlatformError};
        use tokio::sync::mpsc;

        use super::super::*;
        use crate::connect::DemoConnector;
        use crate::covers::DemoCovers;
        use crate::demo::FakeServer;
        use crate::session::Session;
        use crate::store::JsonStore;
        use crate::ui::{AppState, Page, Route};

        type Queue = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

        struct Proxy(Queue);

        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), EventLoopError> {
                Ok(())
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), EventLoopError> {
                self.0
                    .lock()
                    .map_err(|_| EventLoopError::EventLoopTerminated)?
                    .push(event);
                Ok(())
            }
        }

        struct Headless {
            window: Rc<MinimalSoftwareWindow>,
            queue: Queue,
        }

        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
                Ok(self.window.clone())
            }

            fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
                Some(Box::new(Proxy(Arc::clone(&self.queue))))
            }
        }

        fn pump(window: &MinimalSoftwareWindow, queue: &Queue, size: PhysicalSize) {
            let events: Vec<_> = queue
                .lock()
                .map(|mut queue| queue.drain(..).collect())
                .unwrap_or_default();
            for event in events {
                event();
            }
            slint::platform::update_timers_and_animations();
            let mut buffer = vec![Rgb565Pixel::default(); (size.width * size.height) as usize];
            window.draw_if_needed(|renderer| {
                renderer.render(&mut buffer, size.width as usize);
            });
            window.request_redraw();
        }

        #[test]
        fn home_route_renders_shelves_with_lazy_covers_and_plays_albums() {
            let size = PhysicalSize::new(1000, 700);
            let soft = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
            soft.set_size(size);
            let queue = Queue::default();
            let platform = Headless {
                window: Rc::clone(&soft),
                queue: Arc::clone(&queue),
            };
            if let Err(e) = slint::platform::set_platform(Box::new(platform)) {
                eprintln!("skipped: another headless test owns the Slint event loop proxy ({e})");
                return;
            }

            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap_or_else(|e| panic!("runtime: {e}"));
            let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e}"));
            let server = Arc::new(FakeServer::new(2_400));
            let session = Session::default();
            session.set(Some(server.clone()));
            let (commands, mut received) = mpsc::unbounded_channel();
            let ctx = Context {
                rt: rt.handle().clone(),
                commands,
                session,
                store: Arc::new(JsonStore::new(dir.path())),
                connector: Arc::new(DemoConnector),
                covers: Arc::new(DemoCovers),
            };

            let window = AppWindow::new().unwrap_or_else(|e| panic!("window: {e}"));
            crate::ui::nav::install(&window);
            install(&window, &ctx);
            window.show().unwrap_or_else(|e| panic!("show: {e}"));
            let app = window.global::<AppState>();
            app.set_page(Page::Main);
            app.invoke_navigate(Route::Home, "".into());

            let deadline = Instant::now() + Duration::from_secs(10);
            let loaded = |window: &AppWindow| {
                with_model(window, |model| {
                    (0..4).all(|shelf| model.cards(shelf).len() == SHELF_SIZE)
                        && model.cards(0).first().is_some_and(|card| card.has_cover)
                })
                .unwrap_or(false)
            };
            while !loaded(&window) && Instant::now() < deadline {
                pump(&soft, &queue, size);
                std::thread::sleep(Duration::from_millis(5));
            }
            for _ in 0..20 {
                pump(&soft, &queue, size);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(loaded(&window), "home shelves did not load");

            let expected = rt
                .block_on(server.album_list(
                    &Shelf::RecentlyAdded.query(),
                    Paging {
                        start_index: 0,
                        limit: SHELF_SIZE,
                    },
                ))
                .map(|page| page.items)
                .unwrap_or_default();
            let cards = with_model(&window, |model| model.cards(0)).unwrap_or_default();
            let names: Vec<_> = cards.iter().map(|card| card.name.to_string()).collect();
            let expected: Vec<_> = expected.iter().map(|album| album.name.clone()).collect();
            assert_eq!(names, expected);

            let covers = |shelf: usize| {
                with_model(&window, |model| {
                    model.cards(shelf).iter().filter(|c| c.has_cover).count()
                })
                .unwrap_or_default()
            };
            assert!(
                (1..SHELF_SIZE).contains(&covers(0)),
                "only visible cards load covers"
            );
            assert!(!cards[SHELF_SIZE - 1].has_cover);
            assert_eq!(covers(3), 0, "the off-screen shelf has no covers");

            let album_id = cards[0].id.clone();
            window.global::<HomeState>().invoke_play(album_id.clone());
            let command = rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(5), received.recv()).await
            });
            match command {
                Ok(Some(Command::PlaySongs { songs, song_id })) => {
                    assert!(song_id.is_none());
                    assert!(!songs.is_empty());
                    assert!(
                        songs
                            .iter()
                            .all(|song| song.album_id.as_deref() == Some(album_id.as_str()))
                    );
                }
                _ => panic!("expected PlaySongs"),
            }
        }
    }
}
