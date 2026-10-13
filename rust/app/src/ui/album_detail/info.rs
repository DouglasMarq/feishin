use feishin_core::model::AlbumDetail;

use super::super::format_duration;

fn song_count(detail: &AlbumDetail) -> usize {
    match detail.songs.len() {
        0 => detail.album.song_count as usize,
        loaded => loaded,
    }
}

fn total_duration_ms(detail: &AlbumDetail) -> u64 {
    match detail.songs.as_slice() {
        [] => detail.album.duration_ms,
        songs => songs.iter().map(|song| song.duration_ms).sum(),
    }
}

fn song_count_label(count: usize) -> String {
    let noun = match count {
        1 => "song",
        _ => "songs",
    };
    format!("{count} {noun}")
}

pub fn album_info(detail: &AlbumDetail) -> String {
    let album = &detail.album;
    let year = album.year.map(|year| year.to_string());
    let genres = (!album.genres.is_empty()).then(|| album.genres.join(", "));
    year.into_iter()
        .chain(genres)
        .chain([
            song_count_label(song_count(detail)),
            format_duration(total_duration_ms(detail)),
        ])
        .collect::<Vec<_>>()
        .join(" • ")
}

#[cfg(test)]
mod tests {
    use feishin_api::MusicServer;

    use super::*;
    use crate::demo::FakeServer;

    #[tokio::test]
    async fn info_summarizes_the_album() {
        let server = FakeServer::new(24);
        let detail = server.album("album-0").await.expect("album");
        let info = album_info(&detail);
        let year = detail.album.year.unwrap_or_default().to_string();
        assert!(info.starts_with(&year));
        assert!(info.contains("12 songs"));
        assert!(info.ends_with(&format_duration(detail.album.duration_ms)));
    }
}
