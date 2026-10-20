use feishin_youtube::music::{
    CollectionKind, CollectionPage, MusicCollection, MusicShelf, MusicSong, ShelfItem,
};
use serde::{Deserialize, Serialize};

use super::calendar::{MixDay, MixWeek};

pub const MIX_FORMAT_VERSION: u32 = 1;
pub const MIX_ID_PREFIX: &str = "mix:";
const DAILY_ID_PREFIX: &str = "daily-";
const WEEKLY_ID: &str = "weekly";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixSet {
    pub version: u32,
    pub server_id: String,
    pub day: MixDay,
    pub daily: Vec<DailyMix>,
    pub weekly: Option<WeeklyMix>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyMix {
    pub number: u8,
    pub artists: Vec<String>,
    pub songs: Vec<MusicSong>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyMix {
    pub week: MixWeek,
    pub songs: Vec<MusicSong>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixId {
    Daily(u8),
    Weekly,
}

impl MixId {
    pub fn browse_id(self) -> String {
        match self {
            Self::Daily(number) => format!("{MIX_ID_PREFIX}{DAILY_ID_PREFIX}{number}"),
            Self::Weekly => format!("{MIX_ID_PREFIX}{WEEKLY_ID}"),
        }
    }

    pub fn parse(browse_id: &str) -> Option<Self> {
        let name = browse_id.strip_prefix(MIX_ID_PREFIX)?;
        match name.strip_prefix(DAILY_ID_PREFIX) {
            Some(number) => number.parse().ok().map(Self::Daily),
            None => (name == WEEKLY_ID).then_some(Self::Weekly),
        }
    }
}

pub fn is_mix_id(browse_id: &str) -> bool {
    browse_id.starts_with(MIX_ID_PREFIX)
}

impl DailyMix {
    fn title(&self) -> String {
        tr!("Daily Mix {}", self.number)
    }

    fn subtitle(&self) -> String {
        tr!("With {}", self.artists.join(", "))
    }
}

fn weekly_title() -> String {
    tr!("Discover Weekly")
}

fn weekly_subtitle() -> String {
    tr!("New to you, refreshed every Monday")
}

impl MixSet {
    pub fn is_fresh_for(&self, server_id: &str, today: MixDay) -> bool {
        self.version == MIX_FORMAT_VERSION && self.server_id == server_id && self.day == today
    }

    pub fn is_for_server(&self, server_id: &str) -> bool {
        self.version == MIX_FORMAT_VERSION && self.server_id == server_id
    }

    pub fn weekly_for(&self, server_id: &str, week: MixWeek) -> Option<&WeeklyMix> {
        self.weekly
            .as_ref()
            .filter(|weekly| self.is_for_server(server_id) && weekly.week == week)
    }

    pub fn page(&self, id: MixId) -> Option<CollectionPage> {
        let (title, subtitle, songs) = match id {
            MixId::Daily(number) => {
                let mix = self.daily.iter().find(|mix| mix.number == number)?;
                (mix.title(), mix.subtitle(), mix.songs.clone())
            }
            MixId::Weekly => {
                let weekly = self.weekly.as_ref()?;
                (weekly_title(), weekly_subtitle(), weekly.songs.clone())
            }
        };
        Some(CollectionPage {
            title,
            subtitle,
            thumbnail_url: cover_of(&songs),
            songs,
        })
    }

    pub fn shelf(&self) -> Option<MusicShelf> {
        let daily = self.daily.iter().map(|mix| {
            mix_card(
                MixId::Daily(mix.number),
                mix.title(),
                mix.subtitle(),
                &mix.songs,
            )
        });
        let weekly = self.weekly.iter().map(|weekly| {
            mix_card(
                MixId::Weekly,
                weekly_title(),
                weekly_subtitle(),
                &weekly.songs,
            )
        });
        let items: Vec<ShelfItem> = daily.chain(weekly).collect();
        (!items.is_empty()).then(|| MusicShelf {
            title: tr!("Your mixes"),
            items,
        })
    }
}

fn mix_card(id: MixId, title: String, subtitle: String, songs: &[MusicSong]) -> ShelfItem {
    ShelfItem::Collection(MusicCollection {
        browse_id: id.browse_id(),
        kind: CollectionKind::Playlist,
        title,
        subtitle,
        thumbnail_url: cover_of(songs),
    })
}

fn cover_of(songs: &[MusicSong]) -> Option<String> {
    songs.iter().find_map(|song| song.thumbnail_url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str) -> MusicSong {
        MusicSong {
            video_id: id.to_owned(),
            title: format!("Title {id}"),
            artists: vec!["Artist".to_owned()],
            album: None,
            duration_ms: None,
            thumbnail_url: Some(format!("https://example.invalid/{id}.jpg")),
        }
    }

    fn mix_set() -> MixSet {
        MixSet {
            version: MIX_FORMAT_VERSION,
            server_id: "server".into(),
            day: MixDay(100),
            daily: vec![DailyMix {
                number: 1,
                artists: vec!["Velvet Comets".into(), "Iron Verdict".into()],
                songs: vec![song("a"), song("b")],
            }],
            weekly: Some(WeeklyMix {
                week: MixDay(100).week(),
                songs: vec![song("c")],
            }),
        }
    }

    #[test]
    fn mix_ids_round_trip_and_reject_others() {
        assert_eq!(MixId::Daily(3).browse_id(), "mix:daily-3");
        assert_eq!(MixId::parse("mix:daily-3"), Some(MixId::Daily(3)));
        assert_eq!(MixId::parse("mix:weekly"), Some(MixId::Weekly));
        assert_eq!(MixId::parse("mix:daily-x"), None);
        assert_eq!(MixId::parse("VLabc"), None);
        assert!(is_mix_id("mix:anything"));
        assert!(!is_mix_id("MPREb_x"));
    }

    #[test]
    fn mixes_are_fresh_for_their_server_on_their_day_only() {
        let mixes = mix_set();
        assert!(mixes.is_fresh_for("server", MixDay(100)));
        assert!(!mixes.is_fresh_for("server", MixDay(101)));
        assert!(!mixes.is_fresh_for("other", MixDay(100)));
        assert!(mixes.weekly_for("server", MixDay(100).week()).is_some());
        assert!(mixes.weekly_for("server", MixDay(108).week()).is_none());
    }

    #[test]
    fn the_shelf_lists_daily_mixes_then_the_weekly_and_pages_hold_their_songs() {
        let mixes = mix_set();
        let shelf = mixes.shelf().unwrap();
        assert_eq!(shelf.title, "Your mixes");
        let ids: Vec<String> = shelf
            .items
            .iter()
            .filter_map(|item| match item {
                ShelfItem::Collection(collection) => Some(collection.browse_id.clone()),
                ShelfItem::Song(_) => None,
            })
            .collect();
        assert_eq!(ids, ["mix:daily-1", "mix:weekly"]);
        let page = mixes.page(MixId::Daily(1)).unwrap();
        assert_eq!(page.title, "Daily Mix 1");
        assert_eq!(page.subtitle, "With Velvet Comets, Iron Verdict");
        assert_eq!(page.songs.len(), 2);
        assert_eq!(
            page.thumbnail_url.as_deref(),
            Some("https://example.invalid/a.jpg")
        );
        assert!(mixes.page(MixId::Daily(9)).is_none());
    }

    #[test]
    fn mixes_survive_a_json_round_trip() {
        let mixes = mix_set();
        let json = serde_json::to_vec(&mixes).unwrap();
        assert_eq!(serde_json::from_slice::<MixSet>(&json).unwrap(), mixes);
    }
}
