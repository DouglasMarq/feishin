use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::model::{AlbumListQuery, Genre, SongListQuery};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::ui::{AppState, AppWindow, Context, GenreRow, GenresState, show_status};

pub fn genre_song_query(genre_id: &str) -> SongListQuery {
    SongListQuery {
        genre_ids: vec![genre_id.to_owned()],
        ..SongListQuery::default()
    }
}

pub fn genre_album_query(genre_id: &str) -> AlbumListQuery {
    AlbumListQuery {
        genre_ids: vec![genre_id.to_owned()],
        ..AlbumListQuery::default()
    }
}

fn count_label(value: Option<u32>) -> String {
    value.map(|count| count.to_string()).unwrap_or_default()
}

fn genre_row(genre: &Genre) -> GenreRow {
    GenreRow {
        id: genre.id.as_str().into(),
        name: genre.name.as_str().into(),
        albums: count_label(genre.album_count).into(),
        tracks: count_label(genre.song_count).into(),
    }
}

pub fn filter_genres(genres: &[Genre], filter: &str) -> Vec<GenreRow> {
    let filter = filter.trim().to_lowercase();
    genres
        .iter()
        .filter(|genre| filter.is_empty() || genre.name.to_lowercase().contains(&filter))
        .map(genre_row)
        .collect()
}

#[derive(Default)]
pub struct GenreList {
    pub genres: Vec<Genre>,
    pub filter: String,
    generation: u64,
}

impl GenreList {
    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn contains(&self, genre_id: &str) -> bool {
        self.genres.iter().any(|genre| genre.id == genre_id)
    }
}

pub fn show_filtered_genres(window: &AppWindow, list: &GenreList) {
    let rows = filter_genres(&list.genres, &list.filter);
    window
        .global::<GenresState>()
        .set_genres(ModelRc::new(VecModel::from(rows)));
}

pub fn show_genre(window: &AppWindow, list: &GenreList) {
    let state = window.global::<GenresState>();
    let genre_id = window.global::<AppState>().get_detail_id();
    let genre = list
        .genres
        .iter()
        .find(|genre| genre.id == genre_id.as_str());
    state.set_title(
        genre
            .map(|genre| genre.name.as_str())
            .unwrap_or_default()
            .into(),
    );
    state.set_album_count(count_label(genre.and_then(|genre| genre.album_count)).into());
    state.set_track_count(count_label(genre.and_then(|genre| genre.song_count)).into());
}

pub fn fetch_genres(window: &AppWindow, ctx: &Context, list: &Rc<RefCell<GenreList>>) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let generation = list.borrow_mut().next_generation();
    window.global::<GenresState>().set_genres_loading(true);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    ctx.rt.spawn(async move {
        if sender.send(server.genre_list().await).is_err() {
            tracing::debug!("genre list receiver is gone");
        }
    });
    let weak = window.as_weak();
    let list = Rc::clone(list);
    let delivery = slint::spawn_local(async move {
        let (Ok(result), Some(window)) = (receiver.await, weak.upgrade()) else {
            return;
        };
        apply_genres(&window, &mut list.borrow_mut(), generation, result);
    });
    if let Err(error) = delivery {
        tracing::debug!("UI event loop is gone: {error}");
    }
}

fn apply_genres(
    window: &AppWindow,
    list: &mut GenreList,
    generation: u64,
    result: feishin_api::Result<Vec<Genre>>,
) {
    if list.generation != generation {
        return;
    }
    window.global::<GenresState>().set_genres_loading(false);
    match result {
        Ok(genres) => {
            list.genres = genres;
            show_filtered_genres(window, list);
            show_genre(window, list);
        }
        Err(error) => show_status(window, &format!("Could not load genres: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genre(id: &str, name: &str) -> Genre {
        Genre {
            id: id.into(),
            name: name.into(),
            song_count: Some(12),
            album_count: Some(1),
        }
    }

    #[test]
    fn genres_filter_by_name_ignoring_case() {
        let genres = [
            genre("1", "Rock"),
            genre("2", "Post-Rock"),
            genre("3", "Jazz"),
        ];
        let names = |filter| -> Vec<String> {
            filter_genres(&genres, filter)
                .iter()
                .map(|row| row.name.to_string())
                .collect()
        };
        assert_eq!(names(""), ["Rock", "Post-Rock", "Jazz"]);
        assert_eq!(names(" rock "), ["Rock", "Post-Rock"]);
        assert!(names("metal").is_empty());
        assert_eq!(filter_genres(&genres, "jazz")[0].tracks, "12");
    }

    #[test]
    fn genre_queries_filter_by_genre_id() {
        assert_eq!(genre_song_query("g1").genre_ids, ["g1"]);
        assert_eq!(genre_album_query("g1").genre_ids, ["g1"]);
        assert_eq!(genre_song_query("g1").favorite, None);
    }
}
