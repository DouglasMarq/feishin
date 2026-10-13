use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Folder, FolderId};
use slint::{ComponentHandle, Model, SharedString};

use crate::controller::Command;
use crate::demo::FakeServer;
use crate::list_stream::{ListRow, Placement, SongSource};
use crate::ui::test_window::TestWindow;
use crate::ui::{AppState, FolderPlayMode, FolderSongAction, FoldersState, Route};

struct FolderPage {
    view: TestWindow,
    server: Arc<FakeServer>,
}

impl FolderPage {
    fn open() -> Option<Self> {
        let server = Arc::new(FakeServer::new(600));
        let view = TestWindow::open(server.clone())?;
        view.app
            .global::<AppState>()
            .invoke_navigate(Route::Folders, SharedString::default());
        Some(Self { view, server })
    }

    fn state(&self) -> FoldersState<'_> {
        self.view.app.global::<FoldersState>()
    }

    fn crumb_names(&self) -> Vec<String> {
        self.state()
            .get_crumbs()
            .iter()
            .map(|crumb| crumb.name.into())
            .collect()
    }

    fn wait_for_rows(&self, count: usize) -> bool {
        self.view.pump(|| {
            let state = self.state();
            !state.get_loading() && state.get_rows().row_count() == count
        })
    }

    fn open_folder(&self, folder: &Folder) {
        self.state()
            .invoke_open(folder.id.as_str().into(), folder.name.as_str().into());
    }

    fn first_artist_with_albums(&self) -> (Folder, Vec<Folder>) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for artist in self.server.subfolders(&FolderId::Root).await.unwrap() {
                let albums = self.server.subfolders(&artist.folder_id()).await.unwrap();
                if !albums.is_empty() {
                    return (artist, albums);
                }
            }
            panic!("no artist folder has albums");
        })
    }
}

fn assert_root_lists_artist_folders(page: &FolderPage) {
    assert!(page.wait_for_rows(400));
    let state = page.state();
    assert_eq!(page.crumb_names(), ["Folders"]);
    assert_eq!(state.get_name(), "Folders");
    assert_eq!(state.get_summary(), "400 folders");
    let first = state.get_rows().row_data(0).unwrap();
    assert!(first.is_folder);
    assert_eq!(first.folder_id, "folder-artist-0");
}

fn assert_album_rows_are_tracks(page: &FolderPage) {
    let rows = page.state().get_rows();
    let first = rows.row_data(0).unwrap();
    assert!(!first.is_folder);
    assert!(first.track.loaded);
    assert_eq!(first.track.index, 1);
    assert_eq!(page.state().get_summary(), "12 songs");
}

#[test]
fn folders_drill_down_with_breadcrumbs_and_back() {
    let Some(page) = FolderPage::open() else {
        return;
    };
    assert_root_lists_artist_folders(&page);
    let (artist, albums) = page.first_artist_with_albums();

    page.open_folder(&artist);
    assert!(page.wait_for_rows(albums.len()));
    assert_eq!(page.crumb_names(), ["Folders", artist.name.as_str()]);

    page.open_folder(&albums[0]);
    assert!(page.wait_for_rows(12));
    assert_eq!(
        page.crumb_names(),
        ["Folders", artist.name.as_str(), albums[0].name.as_str()]
    );
    assert_album_rows_are_tracks(&page);

    page.view.app.global::<AppState>().invoke_back();
    assert!(page.wait_for_rows(albums.len()));
    assert_eq!(page.crumb_names(), ["Folders", artist.name.as_str()]);

    page.view
        .app
        .global::<AppState>()
        .invoke_navigate(Route::Folders, SharedString::default());
    assert!(page.wait_for_rows(400));
    assert_eq!(page.crumb_names(), ["Folders"]);
}

fn assert_song_rows_play_within_the_album(page: &FolderPage, album: &FolderId) {
    let state = page.state();
    let third_song = state.get_rows().row_data(3).unwrap().track.id.to_string();
    while page.view.next_command().is_some() {}

    state.invoke_play_row(3);
    match page.view.next_command() {
        Some(Command::PlayFromList {
            source: SongSource::Folder(folder),
            song_id,
            row_index,
        }) => {
            assert_eq!(&folder, album);
            assert_eq!(song_id.as_deref(), Some(third_song.as_str()));
            assert_eq!(row_index, ListRow(3));
        }
        _ => panic!("expected a folder play"),
    }

    state.invoke_song_action(3, FolderSongAction::PlayNext);
    assert!(matches!(
        page.view.next_command(),
        Some(Command::AddNext(songs)) if songs.len() == 1 && songs[0].id == third_song
    ));
}

fn assert_folders_stream_recursively(page: &FolderPage, artist: &Folder) {
    let state = page.state();
    state.invoke_play_folder(artist.id.as_str().into(), FolderPlayMode::Last);
    match page.view.next_command() {
        Some(Command::EnqueueFromList {
            source: SongSource::FolderTree(folder),
            placement,
            ..
        }) => {
            assert_eq!(folder, artist.folder_id());
            assert_eq!(placement, Placement::Last);
        }
        _ => panic!("expected a recursive folder enqueue"),
    }

    state.invoke_play_folder(SharedString::default(), FolderPlayMode::Now);
    assert!(matches!(
        page.view.next_command(),
        Some(Command::PlayFromList {
            source: SongSource::FolderTree(FolderId::Root),
            song_id: None,
            ..
        })
    ));
}

#[test]
fn folder_actions_stream_songs_into_the_queue() {
    let Some(page) = FolderPage::open() else {
        return;
    };
    assert!(page.wait_for_rows(400));
    let (artist, albums) = page.first_artist_with_albums();
    page.open_folder(&artist);
    page.open_folder(&albums[0]);
    assert!(page.wait_for_rows(12));

    assert_song_rows_play_within_the_album(&page, &FolderId::directory(albums[0].id.as_str()));
    assert_folders_stream_recursively(&page, &artist);
}
