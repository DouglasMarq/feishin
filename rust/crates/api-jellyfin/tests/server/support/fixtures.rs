use feishin_core::model::Song;
use serde_json::{Value, json};

pub fn user_data(favorite: bool, plays: u32) -> Value {
    json!({
        "IsFavorite": favorite,
        "Key": "k",
        "PlaybackPositionTicks": 0,
        "PlayCount": plays,
        "Played": plays > 0,
        "LastPlayedDate": "2024-02-03T04:05:06.0000000Z"
    })
}

fn media_sources_json() -> Value {
    json!([{
        "Container": "flac",
        "Path": "/music/a/03.flac",
        "Size": 31_000_000,
        "MediaStreams": [
            { "Type": "Video", "Codec": "mjpeg", "BitRate": 1 },
            { "Type": "Audio", "Codec": "flac", "BitRate": 912_345, "SampleRate": 44100 }
        ]
    }])
}

pub fn song_json(id: &str, album_id: &str) -> Value {
    json!({
        "Album": "Album One",
        "AlbumArtist": "Artist A",
        "AlbumArtists": [{ "Id": "ar1", "Name": "Artist A" }],
        "AlbumId": album_id,
        "AlbumPrimaryImageTag": "albtag",
        "ArtistItems": [{ "Id": "ar2", "Name": "Artist B" }],
        "Artists": ["Artist B"],
        "BackdropImageTags": [],
        "ChannelId": null,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [{ "Id": "g1", "Name": "Rock" }],
        "Genres": ["Rock"],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": {},
        "IndexNumber": 3,
        "IsFolder": false,
        "LocationType": "FileSystem",
        "MediaSources": media_sources_json(),
        "MediaType": "Audio",
        "Name": format!("Song {id}"),
        "NormalizationGain": -6.5,
        "ParentIndexNumber": 2,
        "PlaylistItemId": format!("entry-{id}"),
        "PremiereDate": "2010-06-01T00:00:00.0000000Z",
        "ProductionYear": 2009,
        "RunTimeTicks": 2_455_000_000_u64,
        "ServerId": "s",
        "Type": "Audio",
        "UserData": user_data(true, 7)
    })
}

pub fn album_json(id: &str) -> Value {
    json!({
        "AlbumArtist": "Artist A",
        "AlbumArtists": [{ "Id": "ar1", "Name": "Artist A" }],
        "AlbumPrimaryImageTag": "x",
        "ArtistItems": [],
        "Artists": [],
        "ChannelId": null,
        "ChildCount": 12,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [{ "Id": "g1", "Name": "Rock" }],
        "Genres": ["Rock"],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": { "Primary": "primtag" },
        "IsFolder": true,
        "LocationType": "FileSystem",
        "Name": format!("Album {id}"),
        "ProductionYear": 1999,
        "RunTimeTicks": 36_000_000_000_u64,
        "ServerId": "s",
        "Studios": [],
        "Type": "MusicAlbum",
        "UserData": user_data(false, 3)
    })
}

pub fn artist_json(id: &str) -> Value {
    json!({
        "AlbumCount": 4,
        "BackdropImageTags": [],
        "ChannelId": null,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [],
        "Genres": [],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": { "Primary": "t" },
        "LocationType": "FileSystem",
        "Name": format!("Artist {id}"),
        "Overview": "Bio",
        "RunTimeTicks": 0,
        "ServerId": "s",
        "SongCount": 40,
        "Type": "MusicArtist",
        "UserData": user_data(true, 0)
    })
}

pub fn playlist_json(id: &str) -> Value {
    json!({
        "BackdropImageTags": [],
        "ChannelId": null,
        "ChildCount": 2,
        "DateCreated": "2024-01-01T00:00:00.0000000Z",
        "GenreItems": [],
        "Genres": [],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": {},
        "IsFolder": true,
        "LocationType": "FileSystem",
        "MediaType": "Audio",
        "Name": format!("Playlist {id}"),
        "Overview": "Mix",
        "RunTimeTicks": 4_910_000_000_u64,
        "ServerId": "s",
        "Type": "Playlist",
        "UserData": user_data(false, 0)
    })
}

pub fn list(items: Vec<Value>, total: usize) -> Value {
    json!({ "Items": items, "StartIndex": 0, "TotalRecordCount": total })
}

pub fn sample_song(id: &str) -> Song {
    Song {
        id: id.into(),
        server_id: "srv1".into(),
        title: "t".into(),
        artists: vec![],
        album_artists: vec![],
        album: None,
        album_id: None,
        duration_ms: 0,
        track_number: None,
        disc_number: None,
        year: None,
        genres: vec![],
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
