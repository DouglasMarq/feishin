use feishin_core::model::Song;

pub fn blank_song(id: &str, server_id: &str, duration_ms: u64) -> Song {
    Song {
        id: id.to_owned(),
        server_id: server_id.to_owned(),
        title: id.to_owned(),
        artists: Vec::new(),
        album_artists: Vec::new(),
        album: None,
        album_id: None,
        duration_ms,
        track_number: None,
        disc_number: None,
        year: None,
        genres: Vec::new(),
        bitrate_kbps: None,
        play_count: 0,
        user_rating: None,
        favorite: false,
        cover_art_id: None,
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: None,
        created_at: None,
        last_played_at: None,
    }
}
