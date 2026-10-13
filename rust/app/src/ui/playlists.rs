use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, Playlist, Song};
use slint::{ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, SharedString};

use super::{
    AppState, AppWindow, Context, PlaylistPlayMode, PlaylistRow, PlaylistsState, Route,
    format_duration, on_ui, show_status,
};
use crate::controller::Command;

const PAGE_SIZE: usize = 500;
const COVER_SIZE: u32 = 96;
const COVER_CAPACITY: usize = 48;

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
        count => format!("{count} songs"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Now,
    Next,
    Last,
}

impl From<PlaylistPlayMode> for PlayMode {
    fn from(mode: PlaylistPlayMode) -> Self {
        match mode {
            PlaylistPlayMode::Now => Self::Now,
            PlaylistPlayMode::Next => Self::Next,
            PlaylistPlayMode::Last => Self::Last,
        }
    }
}

impl PlayMode {
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

#[derive(Default)]
struct CoverLru {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    requested: HashSet<String>,
}

impl CoverLru {
    fn insert(&mut self, cover_id: String, image: Image) {
        self.requested.remove(&cover_id);
        if self.images.insert(cover_id.clone(), image).is_none() {
            self.order.push_back(cover_id);
        }
        while self.order.len() > COVER_CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }
}

#[derive(Clone)]
struct PlaylistEntry {
    row: PlaylistRow,
    cover_id: Option<String>,
}

pub struct PlaylistsModel {
    entries: RefCell<Vec<PlaylistEntry>>,
    covers: RefCell<CoverLru>,
    generation: Cell<u64>,
    notify: ModelNotify,
    fetch_cover: Box<dyn Fn(String)>,
}

impl PlaylistsModel {
    pub fn new(fetch_cover: impl Fn(String) + 'static) -> Self {
        Self {
            entries: RefCell::default(),
            covers: RefCell::default(),
            generation: Cell::new(0),
            notify: ModelNotify::default(),
            fetch_cover: Box::new(fetch_cover),
        }
    }

    pub fn begin(&self) -> u64 {
        self.generation.set(self.generation.get() + 1);
        self.generation.get()
    }

    pub fn set_playlists(&self, generation: u64, playlists: &[Playlist]) -> bool {
        if generation != self.generation.get() {
            return false;
        }
        *self.entries.borrow_mut() = playlists
            .iter()
            .map(|playlist| PlaylistEntry {
                row: playlist_row(playlist),
                cover_id: playlist.cover_art_id.clone(),
            })
            .collect();
        self.notify.reset();
        true
    }

    pub fn apply_cover(&self, cover_id: String, image: Option<Image>) {
        let Some(image) = image else {
            return;
        };
        let changed: Vec<usize> = self
            .entries
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.cover_id.as_deref() == Some(cover_id.as_str()))
            .map(|(index, _)| index)
            .collect();
        self.covers.borrow_mut().insert(cover_id, image);
        for row in changed {
            self.notify.row_changed(row);
        }
    }

    fn with_cover(&self, mut row: PlaylistRow, cover_id: String) -> PlaylistRow {
        let cached = self.covers.borrow().images.get(&cover_id).cloned();
        match cached {
            Some(image) => {
                row.cover = image;
                row.has_cover = true;
            }
            None => self.request_cover(cover_id),
        }
        row
    }

    fn request_cover(&self, cover_id: String) {
        let first_request = self.covers.borrow_mut().requested.insert(cover_id.clone());
        if first_request {
            (self.fetch_cover)(cover_id);
        }
    }
}

impl Model for PlaylistsModel {
    type Data = PlaylistRow;

    fn row_count(&self) -> usize {
        self.entries.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<PlaylistRow> {
        let entry = self.entries.borrow().get(row).cloned()?;
        let Some(cover_id) = entry.cover_id else {
            return Some(entry.row);
        };
        Some(self.with_cover(entry.row, cover_id))
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn with_model<Output>(
    window: &AppWindow,
    action: impl FnOnce(&PlaylistsModel) -> Output,
) -> Option<Output> {
    let rows = window.global::<PlaylistsState>().get_rows();
    rows.as_any().downcast_ref::<PlaylistsModel>().map(action)
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
            apply_loaded(window, generation, result)
        });
    });
}

fn apply_loaded(window: &AppWindow, generation: u64, result: feishin_api::Result<Vec<Playlist>>) {
    let playlists = result
        .inspect_err(|error| show_status(window, &format!("Could not load playlists: {error}")))
        .unwrap_or_default();
    if with_model(window, |model| model.set_playlists(generation, &playlists)) == Some(true) {
        window.global::<PlaylistsState>().set_loading(false);
    }
}

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
            Err(error) => tracing::debug!("playlist cover {cover_id}: {error:#}"),
        }
    });
}

pub fn play(ctx: &Context, window: &AppWindow, id: &str, mode: PlayMode) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let (id, task_ctx, weak) = (id.to_owned(), ctx.clone(), window.as_weak());
    ctx.rt.spawn(async move {
        match server.playlist_songs(&id).await {
            Ok(songs) if songs.is_empty() => on_ui(&weak, |window| {
                show_status(window, "This playlist is empty");
            }),
            Ok(songs) => task_ctx.send(mode.command(songs)),
            Err(error) => on_ui(&weak, move |window| {
                show_status(window, &format!("Could not load the playlist: {error}"));
            }),
        }
    });
}

fn create_playlist(window: &AppWindow, ctx: &Context, name: &SharedString) {
    let name = name.trim().to_owned();
    let Some(server) = ctx.session.get() else {
        return;
    };
    if name.is_empty() {
        return;
    }
    window.global::<PlaylistsState>().set_busy(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
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
                Err(error) => {
                    show_status(window, &format!("Could not create the playlist: {error}"));
                }
            }
        });
    });
}

fn delete_playlist(window: &AppWindow, ctx: &Context, id: SharedString) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let state = window.global::<PlaylistsState>();
    let name = state.get_delete_name().to_string();
    state.set_busy(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
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
                Err(error) => {
                    show_status(window, &format!("Could not delete the playlist: {error}"));
                }
            }
        });
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
    install_load(window, ctx);
    install_play(window, ctx);
    install_create(window, ctx);
    install_delete(window, ctx);
}

fn install_load(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx);
        }
    });
}

fn install_play(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_play(move |id, mode| {
        if let Some(window) = weak.upgrade() {
            play(&play_ctx, &window, &id, PlayMode::from(mode));
        }
    });
}

fn install_create(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let create_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_create(move |name| {
        if let Some(window) = weak.upgrade() {
            create_playlist(&window, &create_ctx, &name);
        }
    });
}

fn install_delete(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let delete_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_delete(move |id| {
        if let Some(window) = weak.upgrade() {
            delete_playlist(&window, &delete_ctx, id);
        }
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
        let model = PlaylistsModel::new(move |cover_id| sink.borrow_mut().push(cover_id));
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
        assert_eq!(PlayMode::from(PlaylistPlayMode::Next), PlayMode::Next);
        assert_eq!(PlayMode::from(PlaylistPlayMode::Last), PlayMode::Last);
        assert_eq!(PlayMode::from(PlaylistPlayMode::Now), PlayMode::Now);
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
        assert!(playlists.iter().all(|playlist| playlist.song_count > 0));
    }
}
