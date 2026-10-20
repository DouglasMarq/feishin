use feishin_core::model::SongSort;
use feishin_storage::settings::ColumnSetting;
use slint::ComponentHandle;

use crate::ui::{AppWindow, TrackColumn, TracksState};

impl TrackColumn {
    pub const ALL: [Self; 9] = [
        Self::Index,
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Year,
        Self::Plays,
        Self::Duration,
        Self::Favorite,
        Self::Rating,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Year => "year",
            Self::Plays => "plays",
            Self::Duration => "duration",
            Self::Favorite => "favorite",
            Self::Rating => "rating",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|column| column.id() == id)
    }

    pub fn sort(self) -> SongSort {
        match self {
            Self::Artist => SongSort::Artist,
            Self::Album => SongSort::Album,
            Self::Year => SongSort::Year,
            Self::Plays => SongSort::PlayCount,
            Self::Duration => SongSort::Duration,
            Self::Favorite => SongSort::Favorited,
            Self::Rating => SongSort::Rating,
            Self::Index | Self::Title => SongSort::Name,
        }
    }
}

fn width_of(state: &TracksState<'_>, column: TrackColumn) -> f32 {
    match column {
        TrackColumn::Index => state.get_w_index(),
        TrackColumn::Title => state.get_w_title(),
        TrackColumn::Artist => state.get_w_artist(),
        TrackColumn::Album => state.get_w_album(),
        TrackColumn::Year => state.get_w_year(),
        TrackColumn::Plays => state.get_w_plays(),
        TrackColumn::Duration => state.get_w_duration(),
        TrackColumn::Favorite => state.get_w_favorite(),
        TrackColumn::Rating => state.get_w_rating(),
    }
}

fn is_visible(state: &TracksState<'_>, column: TrackColumn) -> bool {
    match column {
        TrackColumn::Index | TrackColumn::Title => true,
        TrackColumn::Artist => state.get_is_artist_shown(),
        TrackColumn::Album => state.get_is_album_shown(),
        TrackColumn::Year => state.get_is_year_shown(),
        TrackColumn::Plays => state.get_is_plays_shown(),
        TrackColumn::Duration => state.get_is_duration_shown(),
        TrackColumn::Favorite => state.get_is_favorite_shown(),
        TrackColumn::Rating => state.get_is_rating_shown(),
    }
}

fn set_width(state: &TracksState<'_>, column: TrackColumn, width: f32) {
    match column {
        TrackColumn::Index => state.set_w_index(width),
        TrackColumn::Title => state.set_w_title(width),
        TrackColumn::Artist => state.set_w_artist(width),
        TrackColumn::Album => state.set_w_album(width),
        TrackColumn::Year => state.set_w_year(width),
        TrackColumn::Plays => state.set_w_plays(width),
        TrackColumn::Duration => state.set_w_duration(width),
        TrackColumn::Favorite => state.set_w_favorite(width),
        TrackColumn::Rating => state.set_w_rating(width),
    }
}

fn set_visible(state: &TracksState<'_>, column: TrackColumn, is_visible: bool) {
    match column {
        TrackColumn::Index | TrackColumn::Title => {}
        TrackColumn::Artist => state.set_is_artist_shown(is_visible),
        TrackColumn::Album => state.set_is_album_shown(is_visible),
        TrackColumn::Year => state.set_is_year_shown(is_visible),
        TrackColumn::Plays => state.set_is_plays_shown(is_visible),
        TrackColumn::Duration => state.set_is_duration_shown(is_visible),
        TrackColumn::Favorite => state.set_is_favorite_shown(is_visible),
        TrackColumn::Rating => state.set_is_rating_shown(is_visible),
    }
}

pub fn columns(window: &AppWindow) -> Vec<ColumnSetting> {
    let state = window.global::<TracksState>();
    TrackColumn::ALL
        .into_iter()
        .map(|column| ColumnSetting {
            id: column.id().to_owned(),
            width_pixels: width_of(&state, column),
            is_visible: is_visible(&state, column),
        })
        .collect()
}

pub(super) fn apply_columns(window: &AppWindow, columns: &[ColumnSetting]) {
    let state = window.global::<TracksState>();
    let known_columns = columns
        .iter()
        .filter(|setting| setting.width_pixels > 0.0)
        .filter_map(|setting| TrackColumn::from_id(&setting.id).map(|column| (column, setting)));
    for (column, setting) in known_columns {
        set_width(&state, column, setting.width_pixels);
        set_visible(&state, column, setting.is_visible);
    }
}
