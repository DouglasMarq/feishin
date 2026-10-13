use std::collections::HashMap;

use feishin_core::model::ArtistRef;
use serde::Deserialize;

use crate::lenient::string;

const ALBUM_ARTIST_ROLE: &str = "albumartist";
const ARTIST_ROLE: &str = "artist";
const REMIXER_ROLE: &str = "remixer";

#[derive(Debug, Default, Deserialize)]
pub(crate) struct NativeReference {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default, deserialize_with = "string")]
    pub name: String,
}

pub(crate) type Participants = HashMap<String, Vec<NativeReference>>;

pub(crate) struct ArtistFields<'item> {
    pub participants: Option<&'item Participants>,
    pub album_artist_id: Option<&'item str>,
    pub album_artist: Option<&'item str>,
    pub artist_id: Option<&'item str>,
    pub artist: Option<&'item str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remixers {
    Include,
    Exclude,
}

pub(crate) struct RelatedArtists {
    pub album_artists: Vec<ArtistRef>,
    pub artists: Vec<ArtistRef>,
}

pub(crate) fn related_artists(fields: &ArtistFields<'_>, remixers: Remixers) -> RelatedArtists {
    let album_artists = participants_in_role(fields.participants, ALBUM_ARTIST_ROLE)
        .unwrap_or_else(|| vec![fallback_album_artist(fields)]);
    let mut artists = participants_in_role(fields.participants, ARTIST_ROLE).unwrap_or_default();
    if artists.is_empty() {
        artists.extend(fallback_artist(fields));
    }
    if remixers == Remixers::Include {
        let remixer_refs =
            participants_in_role(fields.participants, REMIXER_ROLE).unwrap_or_default();
        append_unlisted(&mut artists, remixer_refs);
    }
    RelatedArtists {
        album_artists,
        artists,
    }
}

pub(crate) fn genre_names(genres: Option<&Vec<NativeReference>>) -> Vec<String> {
    genres
        .map(|genres| genres.iter().map(|genre| genre.name.clone()).collect())
        .unwrap_or_default()
}

fn participants_in_role(participants: Option<&Participants>, role: &str) -> Option<Vec<ArtistRef>> {
    participants?
        .get(role)
        .map(|references| artist_refs(references))
}

fn artist_refs(references: &[NativeReference]) -> Vec<ArtistRef> {
    references
        .iter()
        .map(|reference| ArtistRef {
            id: reference.id.clone(),
            name: reference.name.clone(),
        })
        .collect()
}

fn fallback_album_artist(fields: &ArtistFields<'_>) -> ArtistRef {
    ArtistRef {
        id: fields.album_artist_id.unwrap_or_default().to_owned(),
        name: fields.album_artist.unwrap_or_default().to_owned(),
    }
}

fn fallback_artist(fields: &ArtistFields<'_>) -> Option<ArtistRef> {
    match (fields.artist_id, fields.artist) {
        (Some(id), Some(name)) if !id.is_empty() && !name.is_empty() => Some(ArtistRef {
            id: id.to_owned(),
            name: name.to_owned(),
        }),
        _ => None,
    }
}

fn append_unlisted(artists: &mut Vec<ArtistRef>, candidates: Vec<ArtistRef>) {
    for candidate in candidates {
        if !artists.iter().any(|artist| artist.id == candidate.id) {
            artists.push(candidate);
        }
    }
}
