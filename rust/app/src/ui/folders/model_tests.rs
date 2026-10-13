use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::model::{Folder, FolderId, Song};
use slint::Model;

use super::model::{FolderRowsModel, SONG_PAGE_SIZE, SongPageRequest};
use crate::demo::test_song;

type Requests = Rc<RefCell<Vec<SongPageRequest>>>;

fn recording_model() -> (FolderRowsModel, Requests) {
    let requests = Requests::default();
    let sink = Rc::clone(&requests);
    let model = FolderRowsModel::new(move |request| sink.borrow_mut().push(request));
    (model, requests)
}

fn folders(count: usize) -> Vec<Folder> {
    (0..count)
        .map(|index| Folder {
            id: format!("dir-{index}"),
            name: format!("Folder {index}"),
            cover_art_id: None,
        })
        .collect()
}

fn songs_of_page(page: usize) -> Vec<Song> {
    (0..SONG_PAGE_SIZE)
        .map(|offset| test_song(&format!("s{}", page * SONG_PAGE_SIZE + offset), 1000))
        .collect()
}

fn taken_pages(requests: &Requests) -> Vec<usize> {
    requests
        .borrow_mut()
        .drain(..)
        .map(|request| request.page)
        .collect()
}

#[test]
fn folders_come_first_and_songs_page_in_on_demand() {
    let (model, requests) = recording_model();
    let generation = model.reset(FolderId::directory("artist"));
    assert!(model.show(generation, folders(3), 350, songs_of_page(0)));
    assert_eq!(model.row_count(), 353);

    let folder = model.row_data(0).unwrap();
    assert!(folder.is_folder && folder.folder_id == "dir-0");
    let song = model.row_data(4).unwrap();
    assert!(!song.is_folder && song.track.loaded && song.track.id == "s1");
    assert_eq!(model.song_at(4).map(|(position, _)| position), Some(1));
    assert!(model.song_at(1).is_none());

    let placeholder = model.row_data(3 + 150).unwrap();
    assert!(!placeholder.track.loaded);
    assert_eq!(placeholder.track.index, 151);
    assert_eq!(taken_pages(&requests), [1, 2]);
    assert!(model.row_data(353).is_none());
}

#[test]
fn pages_from_an_older_folder_are_ignored_and_failures_back_off() {
    let (model, requests) = recording_model();
    let old = model.reset(FolderId::directory("old"));
    let current = model.reset(FolderId::directory("new"));
    assert!(!model.show(old, folders(1), 10, Vec::new()));
    assert!(model.show(current, Vec::new(), 250, songs_of_page(0)));

    model.row_data(220);
    let pending = requests.borrow_mut().drain(..).collect::<Vec<_>>();
    assert!(
        pending
            .iter()
            .all(|request| request.folder == FolderId::directory("new"))
    );
    let stale = SongPageRequest {
        generation: old,
        ..pending[0].clone()
    };
    model.apply_song_page(&stale, songs_of_page(2));
    assert!(model.song_at(220).is_none());

    assert!(model.song_page_failed(&pending[0]));
    model.row_data(220);
    assert!(taken_pages(&requests).is_empty());
    model.apply_song_page(&pending[0], songs_of_page(2));
    assert_eq!(
        model.song_at(220).map(|(_, song)| song.id),
        Some("s220".into())
    );
}

#[test]
fn scrolling_a_huge_folder_keeps_a_bounded_cache() {
    let (model, requests) = recording_model();
    let generation = model.reset(FolderId::directory("flat"));
    model.show(generation, Vec::new(), 20_000, songs_of_page(0));
    for row in (0..20_000).step_by(50) {
        model.row_data(row);
        let pending: Vec<SongPageRequest> = requests.borrow_mut().drain(..).collect();
        for request in pending {
            model.apply_song_page(&request, songs_of_page(request.page));
        }
    }
    let cached = (0..20_000)
        .filter(|row| model.song_at(*row).is_some())
        .count();
    assert!(cached <= 8 * SONG_PAGE_SIZE, "{cached} songs cached");
    assert!(model.song_at(19_999).is_some());
}
