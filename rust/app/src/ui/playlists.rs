use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, Playlist, Song};
use slint::{ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker};

use super::{
    AppState, AppWindow, Context, PlaylistRow, PlaylistsState, Route, format_duration, on_ui,
    show_status,
};
use crate::controller::Command;

const PAGE_SIZE: usize = 500;
const COVER_SIZE: u32 = 96;
const COVER_CAPACITY: usize = 48;

/// Fetches every playlist, following pages.
pub async fn fetch_all(server: &dyn MusicServer) -> feishin_api::Result<Vec<Playlist>> {
    let mut all = Vec::new();
    loop {
        let paging = Paging {
            start_index: all.len(),
            limit: PAGE_SIZE,
        };
        let page = server.playlist_list(paging).await?;
        let count = page.items.len();
        all.extend(page.items);
        if count < PAGE_SIZE || page.total_count.is_some_and(|total| all.len() >= total) {
            return Ok(all);
        }
    }
}

pub fn song_count_label(count: usize) -> String {
    match count {
        1 => "1 song".to_owned(),
        n => format!("{n} songs"),
    }
}

/// Where a played playlist goes in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Now,
    Next,
    Last,
}

impl PlayMode {
    pub fn parse(how: &str) -> Self {
        match how {
            "next" => Self::Next,
            "last" => Self::Last,
            _ => Self::Now,
        }
    }

    pub fn command(self, songs: Vec<Song>) -> Command {
        match self {
            Self::Now => Command::PlaySongs {
                songs,
                song_id: None,
            },
            Self::Next => Command::AddNext(songs),
            Self::Last => Command::AddLast(songs),
        }
    }
}

fn playlist_row(playlist: &Playlist) -> PlaylistRow {
    PlaylistRow {
        id: playlist.id.as_str().into(),
        name: playlist.name.as_str().into(),
        songs: playlist.song_count.to_string().into(),
        duration: format_duration(playlist.duration_ms).into(),
        owner: playlist.owner.as_deref().unwrap_or_default().into(),
        cover: Image::default(),
        has_cover: false,
    }
}

/// Bounded cache of decoded covers keyed by cover art id.
#[derive(Default)]
struct CoverLru {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    requested: HashSet<String>,
}

impl CoverLru {
    fn insert(&mut self, id: String, image: Image) {
        self.requested.remove(&id);
        if self.images.insert(id.clone(), image).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > COVER_CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }
}

/// Playlist rows whose covers are fetched lazily when a row becomes visible.
pub struct PlaylistsModel {
    rows: RefCell<Vec<(PlaylistRow, Option<String>)>>,
    covers: RefCell<CoverLru>,
    generation: Cell<u64>,
    notify: ModelNotify,
    fetch_cover: Box<dyn Fn(String)>,
}

impl PlaylistsModel {
    pub fn new(fetch_cover: impl Fn(String) + 'static) -> Self {
        Self {
            rows: RefCell::default(),
            covers: RefCell::default(),
            generation: Cell::new(0),
            notify: ModelNotify::default(),
            fetch_cover: Box::new(fetch_cover),
        }
    }

    /// Starts a new load and returns its generation.
    pub fn begin(&self) -> u64 {
        self.generation.set(self.generation.get() + 1);
        self.generation.get()
    }

    pub fn set_playlists(&self, generation: u64, playlists: &[Playlist]) -> bool {
        if generation != self.generation.get() {
            return false;
        }
        *self.rows.borrow_mut() = playlists
            .iter()
            .map(|p| (playlist_row(p), p.cover_art_id.clone()))
            .collect();
        self.notify.reset();
        true
    }

    pub fn apply_cover(&self, cover_id: String, image: Option<Image>) {
        let Some(image) = image else {
            return;
        };
        let changed: Vec<usize> = self
            .rows
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, (_, id))| id.as_deref() == Some(cover_id.as_str()))
            .map(|(i, _)| i)
            .collect();
        self.covers.borrow_mut().insert(cover_id, image);
        for row in changed {
            self.notify.row_changed(row);
        }
    }
}

impl Model for PlaylistsModel {
    type Data = PlaylistRow;

    fn row_count(&self) -> usize {
        self.rows.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<PlaylistRow> {
        let (mut data, cover_id) = self.rows.borrow().get(row).cloned()?;
        let Some(cover_id) = cover_id else {
            return Some(data);
        };
        let mut covers = self.covers.borrow_mut();
        if let Some(image) = covers.images.get(&cover_id) {
            data.cover = image.clone();
            data.has_cover = true;
        } else if covers.requested.insert(cover_id.clone()) {
            drop(covers);
            (self.fetch_cover)(cover_id);
        }
        Some(data)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn with_model<R>(window: &AppWindow, f: impl FnOnce(&PlaylistsModel) -> R) -> Option<R> {
    let rows = window.global::<PlaylistsState>().get_rows();
    rows.as_any().downcast_ref::<PlaylistsModel>().map(f)
}

fn load(window: &AppWindow, ctx: &Context) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(generation) = with_model(window, PlaylistsModel::begin) else {
        return;
    };
    window.global::<PlaylistsState>().set_loading(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = fetch_all(server.as_ref()).await;
        on_ui(&weak, move |window| {
            let playlists = match result {
                Ok(playlists) => playlists,
                Err(e) => {
                    show_status(window, &format!("Could not load playlists: {e}"));
                    Vec::new()
                }
            };
            if with_model(window, |model| model.set_playlists(generation, &playlists)) == Some(true)
            {
                window.global::<PlaylistsState>().set_loading(false);
            }
        });
    });
}

/// Reloads the list if it is on screen, e.g. after a playlist was created elsewhere.
pub fn refresh(window: &AppWindow) {
    if window.global::<AppState>().get_route() == Route::Playlists {
        window.global::<PlaylistsState>().invoke_load("".into());
    }
}

fn fetch_cover(weak: slint::Weak<AppWindow>, ctx: &Context, cover_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        match covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await {
            Ok(cover) => on_ui(&weak, move |window| {
                let image = super::player::to_image(&cover);
                with_model(window, |model| model.apply_cover(cover_id, image));
            }),
            Err(e) => tracing::debug!("playlist cover {cover_id}: {e:#}"),
        }
    });
}

/// Fetches a playlist's songs and sends them to the queue.
pub fn play(ctx: &Context, window: &AppWindow, id: &str, mode: PlayMode) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let (id, ctx_task, weak) = (id.to_owned(), ctx.clone(), window.as_weak());
    ctx.rt.spawn(async move {
        match server.playlist_songs(&id).await {
            Ok(songs) if songs.is_empty() => on_ui(&weak, |window| {
                show_status(window, "This playlist is empty");
            }),
            Ok(songs) => ctx_task.send(mode.command(songs)),
            Err(e) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not load the playlist: {e}"));
            }),
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<PlaylistsState>();
    let weak = window.as_weak();
    let cover_ctx = ctx.clone();
    state.set_rows(ModelRc::new(Rc::new(PlaylistsModel::new(
        move |cover_id| {
            fetch_cover(weak.clone(), &cover_ctx, cover_id);
        },
    ))));

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    state.on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx);
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play(move |id, how| {
        if let Some(window) = weak.upgrade() {
            play(&play_ctx, &window, &id, PlayMode::parse(&how));
        }
    });

    let weak = window.as_weak();
    let create_ctx = ctx.clone();
    state.on_create(move |name| {
        let name = name.trim().to_owned();
        let (Some(window), Some(server)) = (weak.upgrade(), create_ctx.session.get()) else {
            return;
        };
        if name.is_empty() {
            return;
        }
        window.global::<PlaylistsState>().set_busy(true);
        let weak = weak.clone();
        create_ctx.rt.spawn(async move {
            let result = server.create_playlist(&name, &[]).await;
            on_ui(&weak, move |window| {
                let state = window.global::<PlaylistsState>();
                state.set_busy(false);
                match result {
                    Ok(playlist) => {
                        state.set_creating(false);
                        show_status(window, &format!("Created playlist \"{}\"", playlist.name));
                        refresh(window);
                    }
                    Err(e) => show_status(window, &format!("Could not create the playlist: {e}")),
                }
            });
        });
    });

    let weak = window.as_weak();
    let delete_ctx = ctx.clone();
    state.on_delete(move |id| {
        let (Some(window), Some(server)) = (weak.upgrade(), delete_ctx.session.get()) else {
            return;
        };
        let state = window.global::<PlaylistsState>();
        let name = state.get_delete_name().to_string();
        state.set_busy(true);
        let weak = weak.clone();
        delete_ctx.rt.spawn(async move {
            let result = server.delete_playlist(&id).await;
            on_ui(&weak, move |window| {
                let state = window.global::<PlaylistsState>();
                state.set_busy(false);
                state.set_delete_id("".into());
                match result {
                    Ok(()) => {
                        show_status(window, &format!("Deleted playlist \"{name}\""));
                        refresh(window);
                    }
                    Err(e) => show_status(window, &format!("Could not delete the playlist: {e}")),
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::demo::FakeServer;

    fn playlist(id: &str, cover: Option<&str>) -> Playlist {
        Playlist {
            id: id.into(),
            server_id: "demo".into(),
            name: id.into(),
            comment: None,
            owner: Some("me".into()),
            public: false,
            song_count: 3,
            duration_ms: 185_000,
            cover_art_id: cover.map(Into::into),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn rows_map_playlists() {
        let row = playlist_row(&playlist("a", None));
        assert_eq!(row.songs, "3");
        assert_eq!(row.duration, "3:05");
        assert_eq!(row.owner, "me");
        assert_eq!(song_count_label(1), "1 song");
        assert_eq!(song_count_label(4), "4 songs");
    }

    #[test]
    fn covers_are_requested_once_and_stale_loads_ignored() {
        let requested = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&requested);
        let model = PlaylistsModel::new(move |id| sink.borrow_mut().push(id));
        let stale = model.begin();
        let current = model.begin();
        assert!(!model.set_playlists(stale, &[playlist("old", None)]));
        assert!(model.set_playlists(current, &[playlist("a", Some("c1")), playlist("b", None)]));
        assert_eq!(model.row_count(), 2);

        model.row_data(0);
        model.row_data(0);
        model.row_data(1);
        assert_eq!(*requested.borrow(), ["c1"]);

        model.apply_cover("c1".into(), Some(Image::default()));
        assert!(model.row_data(0).is_some_and(|row| row.has_cover));
        assert!(model.row_data(1).is_some_and(|row| !row.has_cover));
    }

    #[test]
    fn play_modes_map_to_commands() {
        assert_eq!(PlayMode::parse("next"), PlayMode::Next);
        assert_eq!(PlayMode::parse("last"), PlayMode::Last);
        assert_eq!(PlayMode::parse("now"), PlayMode::Now);
        assert!(matches!(
            PlayMode::Now.command(Vec::new()),
            Command::PlaySongs { song_id: None, .. }
        ));
        assert!(matches!(
            PlayMode::Last.command(Vec::new()),
            Command::AddLast(_)
        ));
    }

    #[tokio::test]
    async fn fetch_all_reads_the_demo_playlists() {
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::new(500));
        let playlists = fetch_all(server.as_ref()).await.unwrap_or_default();
        assert!(!playlists.is_empty());
        assert!(playlists.iter().all(|p| p.song_count > 0));
    }
}
