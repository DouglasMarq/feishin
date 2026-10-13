use super::library::DemoLibrary;

pub(super) struct ArtistAlbums {
    albums_by_artist: Vec<Vec<usize>>,
}

impl ArtistAlbums {
    pub(super) fn index(library: DemoLibrary) -> Self {
        let mut albums_by_artist = vec![Vec::new(); library.artist_count()];
        for album in 0..library.album_count() {
            if let Some(albums) = albums_by_artist.get_mut(library.album_artist(album)) {
                albums.push(album);
            }
        }
        Self { albums_by_artist }
    }

    pub(super) fn albums_of(&self, artist: usize) -> &[usize] {
        self.albums_by_artist.get(artist).map_or(&[], Vec::as_slice)
    }
}
