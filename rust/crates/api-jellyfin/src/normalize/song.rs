use feishin_core::model::{ReplayGain, Song};

use super::{artist_refs, credited_artists, names, primary_image, release_year, ticks_to_ms};
use crate::dto;

const AUDIO_STREAM_KIND: &str = "Audio";
const REPLAY_GAIN_REFERENCE_LUFS: f32 = -18.0;

pub fn song(item: dto::Song, server_id: &str) -> Song {
    let source = item.media_sources.first();
    let user_data = item.user_data.as_ref();
    Song {
        server_id: server_id.to_owned(),
        title: item.name.clone(),
        artists: credited_artists(&item.artist_items, &item.album_artists),
        album_artists: artist_refs(&item.album_artists),
        album: item.album.clone(),
        album_id: item.album_id.clone(),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        track_number: item.index_number,
        disc_number: Some(disc_number(item.parent_index_number)),
        year: release_year(item.premiere_date.as_deref(), item.production_year),
        genres: names(&item.genre_items),
        bitrate_kbps: source.and_then(audio_bitrate_kbps),
        play_count: user_data.map_or(0, |data| data.play_count),
        user_rating: None,
        favorite: user_data.is_some_and(|data| data.is_favorite),
        cover_art_id: song_cover_art_id(&item),
        replay_gain: replay_gain(&item),
        path: source.and_then(|source| source.path.clone()),
        size_bytes: source.and_then(|source| source.size),
        container: source.and_then(|source| source.container.clone()),
        created_at: item.date_created.clone(),
        last_played_at: user_data.and_then(|data| data.last_played_date.clone()),
        id: item.id,
    }
}

fn disc_number(parent_index_number: Option<u32>) -> u32 {
    parent_index_number.filter(|disc| *disc > 0).unwrap_or(1)
}

fn audio_bitrate_kbps(source: &dto::MediaSource) -> Option<u32> {
    source
        .media_streams
        .iter()
        .find(|stream| stream.kind == AUDIO_STREAM_KIND)
        .and_then(|stream| stream.bit_rate)
        .map(|bits_per_second| u32::try_from(bits_per_second / 1000).unwrap_or(u32::MAX))
}

fn song_cover_art_id(item: &dto::Song) -> Option<String> {
    primary_image(&item.id, item.image_tags.as_ref()).or_else(|| {
        item.album_primary_image_tag
            .as_ref()
            .and(item.album_id.clone())
    })
}

fn replay_gain(item: &dto::Song) -> Option<ReplayGain> {
    let track_gain_db = item
        .normalization_gain
        .or(item.lufs.map(|lufs| REPLAY_GAIN_REFERENCE_LUFS - lufs));
    if track_gain_db.is_none() && item.album_normalization_gain.is_none() {
        return None;
    }
    Some(ReplayGain {
        track_gain_db,
        album_gain_db: item.album_normalization_gain,
        ..ReplayGain::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_falls_back_to_lufs() {
        let item = dto::Song {
            lufs: Some(-10.0),
            ..dto::Song::default()
        };
        let gain = replay_gain(&item).unwrap();
        assert_eq!(gain.track_gain_db, Some(-8.0));
        assert_eq!(gain.album_gain_db, None);
        assert!(replay_gain(&dto::Song::default()).is_none());
    }
}
