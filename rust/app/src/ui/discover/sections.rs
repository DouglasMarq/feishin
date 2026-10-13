use std::collections::{HashMap, HashSet};

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::MoodCategory;

use crate::discover::{DiscoverFeed, FeedRequest};
use crate::ui::DiscoverSection;

#[derive(Default)]
pub struct Sections {
    pub section: Section,
    pub chart_country: Option<String>,
    pub mood: Option<usize>,
    settings: Option<YouTubeSettings>,
    feeds: HashMap<FeedRequest, DiscoverFeed>,
    errors: HashMap<FeedRequest, String>,
    in_flight: HashSet<FeedRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    ForYou,
    NewReleases,
    Charts,
    Moods,
}

impl Section {
    pub fn from_slint(section: DiscoverSection) -> Self {
        match section {
            DiscoverSection::ForYou => Self::ForYou,
            DiscoverSection::NewReleases => Self::NewReleases,
            DiscoverSection::Charts => Self::Charts,
            DiscoverSection::Moods => Self::Moods,
        }
    }

    pub fn to_slint(self) -> DiscoverSection {
        match self {
            Self::ForYou => DiscoverSection::ForYou,
            Self::NewReleases => DiscoverSection::NewReleases,
            Self::Charts => DiscoverSection::Charts,
            Self::Moods => DiscoverSection::Moods,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ForYou => "your home",
            Self::NewReleases => "new releases",
            Self::Charts => "the charts",
            Self::Moods => "moods and genres",
        }
    }
}

impl Sections {
    pub fn use_settings(&mut self, settings: YouTubeSettings) -> bool {
        if self.settings.as_ref() == Some(&settings) {
            return false;
        }
        self.feeds.clear();
        self.errors.clear();
        self.settings = Some(settings);
        true
    }

    pub fn settings(&self) -> YouTubeSettings {
        self.settings.clone().unwrap_or_default()
    }

    pub fn moods(&self) -> &[MoodCategory] {
        self.feeds
            .get(&FeedRequest::Moods)
            .map(|feed| feed.moods.as_slice())
            .unwrap_or_default()
    }

    pub fn shelves_request(&self) -> Option<FeedRequest> {
        match self.section {
            Section::ForYou => Some(FeedRequest::ForYou),
            Section::NewReleases => Some(FeedRequest::NewReleases),
            Section::Charts => self
                .chart_country
                .clone()
                .map(|country| FeedRequest::Charts { country }),
            Section::Moods => self
                .mood
                .and_then(|index| self.moods().get(index))
                .map(|mood| FeedRequest::Mood {
                    params: mood.params.clone(),
                }),
        }
    }

    pub fn wanted(&self) -> Vec<FeedRequest> {
        let moods = (self.section == Section::Moods).then_some(FeedRequest::Moods);
        moods.into_iter().chain(self.shelves_request()).collect()
    }

    pub fn begin(&mut self, request: &FeedRequest) -> bool {
        if self.feeds.contains_key(request) || self.in_flight.contains(request) {
            return false;
        }
        self.errors.remove(request);
        self.in_flight.insert(request.clone())
    }

    pub fn finish(&mut self, request: FeedRequest, result: Result<DiscoverFeed, String>) {
        self.in_flight.remove(&request);
        match result {
            Ok(feed) => {
                self.feeds.insert(request, feed);
            }
            Err(error) => {
                self.errors.insert(request, error);
            }
        }
    }

    pub fn forget_wanted(&mut self) {
        for request in self.wanted() {
            if request != FeedRequest::Moods || self.mood.is_none() {
                self.feeds.remove(&request);
            }
            self.errors.remove(&request);
        }
    }

    pub fn feed(&self, request: &FeedRequest) -> Option<&DiscoverFeed> {
        self.feeds.get(request)
    }

    pub fn is_loading(&self) -> bool {
        self.wanted()
            .iter()
            .any(|request| self.in_flight.contains(request))
    }

    pub fn error(&self) -> Option<&str> {
        self.wanted()
            .iter()
            .find_map(|request| self.errors.get(request))
            .map(String::as_str)
    }

    pub fn is_wanted(&self, request: &FeedRequest) -> bool {
        self.wanted().contains(request)
    }
}

#[cfg(test)]
mod tests {
    use feishin_youtube::music::MusicShelf;

    use super::*;
    use crate::discover::FeedNotice;

    fn feed_with_moods() -> DiscoverFeed {
        DiscoverFeed {
            moods: vec![MoodCategory {
                title: "Chill".into(),
                params: "chill".into(),
            }],
            ..DiscoverFeed::default()
        }
    }

    #[test]
    fn each_section_wants_its_own_feeds() {
        let mut sections = Sections::default();
        assert_eq!(sections.wanted(), [FeedRequest::ForYou]);
        sections.section = Section::Charts;
        assert!(sections.wanted().is_empty(), "no country yet");
        sections.chart_country = Some("BR".into());
        assert_eq!(
            sections.wanted(),
            [FeedRequest::Charts {
                country: "BR".into()
            }]
        );
        sections.section = Section::Moods;
        assert_eq!(sections.wanted(), [FeedRequest::Moods]);
        sections.finish(FeedRequest::Moods, Ok(feed_with_moods()));
        sections.mood = Some(0);
        assert_eq!(
            sections.wanted(),
            [
                FeedRequest::Moods,
                FeedRequest::Mood {
                    params: "chill".into()
                }
            ]
        );
    }

    #[test]
    fn feeds_load_once_and_failures_are_remembered_until_retried() {
        let mut sections = Sections::default();
        assert!(sections.begin(&FeedRequest::ForYou));
        assert!(!sections.begin(&FeedRequest::ForYou));
        assert!(sections.is_loading());
        sections.finish(FeedRequest::ForYou, Err("offline".into()));
        assert_eq!(sections.error(), Some("offline"));
        assert!(sections.begin(&FeedRequest::ForYou));
        assert_eq!(sections.error(), None);
        let feed = DiscoverFeed::of_shelves(Vec::<MusicShelf>::new(), FeedNotice::None);
        sections.finish(FeedRequest::ForYou, Ok(feed));
        assert!(!sections.begin(&FeedRequest::ForYou), "cached");
        sections.forget_wanted();
        assert!(sections.begin(&FeedRequest::ForYou));
    }

    #[test]
    fn changed_settings_drop_cached_feeds() {
        let mut sections = Sections::default();
        assert!(sections.use_settings(YouTubeSettings::default()));
        sections.finish(FeedRequest::NewReleases, Ok(DiscoverFeed::default()));
        assert!(!sections.use_settings(YouTubeSettings::default()));
        assert!(sections.feed(&FeedRequest::NewReleases).is_some());
        let signed_in = YouTubeSettings {
            cookies_from_browser: Some(feishin_youtube::CookieBrowser::Brave),
            ..YouTubeSettings::default()
        };
        assert!(sections.use_settings(signed_in));
        assert!(sections.feed(&FeedRequest::NewReleases).is_none());
    }

    #[test]
    fn refreshing_a_mood_keeps_the_mood_list() {
        let mut sections = Sections {
            section: Section::Moods,
            ..Sections::default()
        };
        sections.finish(FeedRequest::Moods, Ok(feed_with_moods()));
        sections.mood = Some(0);
        sections.forget_wanted();
        assert_eq!(sections.moods().len(), 1);
    }
}
