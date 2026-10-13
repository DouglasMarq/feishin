use std::hash::{BuildHasher, RandomState};
use std::time::SystemTime;

use feishin_core::model::{Page, Song};

use super::fetching::StreamStop;
use super::pages::{KeptSongs, PageNumber};
use super::rows::ListRow;
use super::runner::StreamRunner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamStart {
    Row {
        row: ListRow,
        song_id: Option<String>,
    },
    FirstRow,
    RandomRow,
}

impl StreamRunner {
    pub(super) async fn start_row(
        &self,
        start: StreamStart,
        first_row: ListRow,
    ) -> Result<(ListRow, Option<String>), StreamStop> {
        match start {
            StreamStart::Row { row, song_id } => Ok((row, song_id)),
            StreamStart::FirstRow => Ok((first_row, None)),
            StreamStart::RandomRow => Ok((self.random_row(first_row).await?, None)),
        }
    }

    async fn random_row(&self, first_row: ListRow) -> Result<ListRow, StreamStop> {
        let first_page = PageNumber::of(first_row);
        let fetched = self.fetch(first_page).await?;
        let row_count = fetched
            .total_count
            .unwrap_or_else(|| first_page.first_row().0 + fetched.items.len());
        let candidates = self.listed.clamped(row_count);
        let offset = random_below(candidates.row_count());
        Ok(candidates.nth_row(offset).unwrap_or(first_row))
    }

    pub(super) async fn fetch_start_page(
        &self,
        row: ListRow,
        first_row: ListRow,
    ) -> Result<(ListRow, Page<Song>), StreamStop> {
        let fetched = self.fetch(PageNumber::of(row)).await?;
        if !fetched.items.is_empty() || PageNumber::of(row) == PageNumber::of(first_row) {
            return Ok((row, fetched));
        }
        tracing::debug!(
            "row {} is past the end of the list, starting at its top",
            row.0
        );
        Ok((first_row, self.fetch(PageNumber::of(first_row)).await?))
    }
}

pub fn start_index_in(kept: &KeptSongs, row: ListRow, song_id: Option<&str>) -> usize {
    let at_row = kept.rows.iter().position(|kept_row| *kept_row == row);
    let Some(song_id) = song_id else {
        return at_row.unwrap_or(0);
    };
    let has_song = |index: &usize| {
        kept.songs
            .get(*index)
            .is_some_and(|song| song.id == song_id)
    };
    at_row
        .filter(has_song)
        .or_else(|| kept.songs.iter().position(|song| song.id == song_id))
        .unwrap_or_else(|| {
            tracing::debug!("song {song_id} is not on its page anymore, starting at the page top");
            0
        })
}

fn random_below(bound: usize) -> usize {
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let random = RandomState::new().hash_one(seed);
    let bound = u64::try_from(bound).unwrap_or(u64::MAX);
    random
        .checked_rem(bound)
        .and_then(|offset| usize::try_from(offset).ok())
        .unwrap_or(0)
}
