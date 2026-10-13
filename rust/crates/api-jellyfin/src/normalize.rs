use feishin_core::model::{
    Album, Artist, ArtistRef, Genre, MusicFolder, Playlist, ReplayGain, Song,
};

use crate::dto;

pub const TICKS_PER_MS: u64 = 10_000;

fn ticks_to_ms(ticks: Option<u64>) -> u64 {
    ticks.unwrap_or(0) / TICKS_PER_MS
}

fn refs(items: &[dto::NamedItem]) -> Vec<ArtistRef> {
    items
        .iter()
        .map(|item| ArtistRef {
            id: item.id.clone(),
            name: item.name.clone(),
        })
        .collect()
}

fn names(items: &[dto::NamedItem]) -> Vec<String> {
    items.iter().map(|item| item.name.clone()).collect()
}

fn primary_image(id: &str, tags: Option<&dto::ImageTags>) -> Option<String> {
    tags.and_then(|tags| tags.primary.as_ref())
        .map(|_| id.to_owned())
}

fn release_year(premiere_date: Option<&str>, production_year: Option<i32>) -> Option<i32> {
    premiere_date
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse::<i32>().ok())
        .filter(|year| *year > 0)
        .or(production_year.filter(|year| *year > 0))
}

fn replay_gain(item: &dto::Song) -> Option<ReplayGain> {
    let track = item
        .normalization_gain
        .or(item.lufs.map(|lufs| -18.0 - lufs));
    if track.is_none() && item.album_normalization_gain.is_none() {
        return None;
    }
    Some(ReplayGain {
        track_gain_db: track,
        album_gain_db: item.album_normalization_gain,
        ..ReplayGain::default()
    })
}

pub fn song(item: dto::Song, server_id: &str) -> Song {
    let source = item.media_sources.first();
    let bitrate_kbps = source
        .and_then(|source| source.media_streams.iter().find(|s| s.kind == "Audio"))
        .and_then(|stream| stream.bit_rate)
        .map(|bits| u32::try_from(bits / 1000).unwrap_or(u32::MAX));
    let cover_art_id = primary_image(&item.id, item.image_tags.as_ref()).or_else(|| {
        item.album_primary_image_tag
            .as_ref()
            .and(item.album_id.clone())
    });
    let artists = if item.artist_items.is_empty() {
        refs(&item.album_artists)
    } else {
        refs(&item.artist_items)
    };
    let user_data = item.user_data.as_ref();

    Song {
        server_id: server_id.to_owned(),
        title: item.name.clone(),
        artists,
        album_artists: refs(&item.album_artists),
        album: item.album.clone(),
        album_id: item.album_id.clone(),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        track_number: item.index_number,
        disc_number: Some(item.parent_index_number.filter(|n| *n > 0).unwrap_or(1)),
        year: release_year(item.premiere_date.as_deref(), item.production_year),
        genres: names(&item.genre_items),
        bitrate_kbps,
        play_count: user_data.map_or(0, |data| data.play_count),
        user_rating: None,
        favorite: user_data.is_some_and(|data| data.is_favorite),
        cover_art_id,
        replay_gain: replay_gain(&item),
        path: source.and_then(|source| source.path.clone()),
        size_bytes: source.and_then(|source| source.size),
        container: source.and_then(|source| source.container.clone()),
        created_at: item.date_created.clone(),
        last_played_at: user_data.and_then(|data| data.last_played_date.clone()),
        id: item.id,
    }
}

pub fn album(item: dto::Album, server_id: &str) -> Album {
    let artists = if item.artist_items.is_empty() {
        refs(&item.album_artists)
    } else {
        refs(&item.artist_items)
    };
    let user_data = item.user_data.as_ref();

    Album {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        album_artists: refs(&item.album_artists),
        artists,
        year: release_year(item.premiere_date.as_deref(), item.production_year),
        genres: names(&item.genre_items),
        song_count: item.child_count.unwrap_or(0),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        play_count: user_data.map_or(0, |data| data.play_count),
        user_rating: None,
        favorite: user_data.is_some_and(|data| data.is_favorite),
        created_at: item.date_created.clone(),
        last_played_at: user_data.and_then(|data| data.last_played_date.clone()),
        id: item.id,
    }
}

pub fn artist(item: dto::AlbumArtist, server_id: &str) -> Artist {
    Artist {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        album_count: item.album_count,
        song_count: item.song_count,
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        favorite: item.user_data.as_ref().is_some_and(|data| data.is_favorite),
        user_rating: None,
        biography: item.overview.filter(|overview| !overview.is_empty()),
        id: item.id,
    }
}

pub fn playlist(item: dto::Playlist, server_id: &str) -> Playlist {
    Playlist {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        comment: item.overview.filter(|overview| !overview.is_empty()),
        owner: None,
        public: false,
        song_count: item.child_count.unwrap_or(0),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        created_at: item.date_created,
        updated_at: None,
        id: item.id,
    }
}

pub fn genre(item: dto::Genre) -> Genre {
    Genre {
        id: item.id,
        name: item.name,
        song_count: item.song_count,
        album_count: item.album_count,
    }
}

pub fn music_folder(item: dto::MusicFolder) -> MusicFolder {
    MusicFolder {
        id: item.id,
        name: item.name,
    }
}

/// Synced Jellyfin lyrics become LRC; unsynced ones are joined as plain text.
pub fn lyrics(lyrics: dto::Lyrics) -> Option<String> {
    let lines = lyrics.lyrics;
    if lines.is_empty() {
        return None;
    }
    if lines[0].start.is_none() {
        return Some(
            lines
                .into_iter()
                .map(|line| line.text)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    Some(
        lines
            .into_iter()
            .map(|line| {
                let ms = ticks_to_ms(line.start);
                let minutes = ms / 60_000;
                let seconds = (ms % 60_000) / 1000;
                let centis = (ms % 1000) / 10;
                format!("[{minutes:02}:{seconds:02}.{centis:02}]{}", line.text)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_prefers_premiere_date() {
        assert_eq!(
            release_year(Some("2019-05-01T00:00:00Z"), Some(2001)),
            Some(2019)
        );
        assert_eq!(release_year(None, Some(2001)), Some(2001));
        assert_eq!(release_year(Some("bad"), Some(0)), None);
    }

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

    #[test]
    fn synced_lyrics_become_lrc() {
        let lyrics = dto::Lyrics {
            lyrics: vec![
                dto::LyricLine {
                    start: Some(0),
                    text: "one".into(),
                },
                dto::LyricLine {
                    start: Some(615_300_000),
                    text: "two".into(),
                },
            ],
        };
        assert_eq!(
            super::lyrics(lyrics).unwrap(),
            "[00:00.00]one\n[01:01.53]two"
        );
    }

    #[test]
    fn unsynced_lyrics_are_plain() {
        let lyrics = dto::Lyrics {
            lyrics: vec![
                dto::LyricLine {
                    start: None,
                    text: "a".into(),
                },
                dto::LyricLine {
                    start: None,
                    text: "b".into(),
                },
            ],
        };
        assert_eq!(super::lyrics(lyrics).unwrap(), "a\nb");
        assert!(super::lyrics(dto::Lyrics::default()).is_none());
    }
}
