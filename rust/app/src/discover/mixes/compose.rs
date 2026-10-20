use std::collections::{HashMap, HashSet};

use feishin_core::random::SplitMix64;
use feishin_youtube::music::MusicSong;

use crate::discover::LibraryPresence;
use crate::discover::names::normalized_name;

pub const MIX_LENGTH: usize = 30;
const DAILY_SONGS_PER_ARTIST: usize = 3;
const WEEKLY_SONGS_PER_ARTIST: usize = 2;
const FAMILIAR_IN_A_ROW: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub song: MusicSong,
    pub presence: LibraryPresence,
}

pub fn compose_daily(
    seed_songs: &[MusicSong],
    candidates: &[Candidate],
    shuffle_seed: u64,
) -> Vec<MusicSong> {
    let (library, new): (Vec<&Candidate>, Vec<&Candidate>) = candidates
        .iter()
        .partition(|candidate| candidate.presence == LibraryPresence::InLibrary);
    let mut random = SplitMix64::from_seed(shuffle_seed);
    let mut familiar: Vec<&MusicSong> = seed_songs
        .iter()
        .chain(library.into_iter().map(|candidate| &candidate.song))
        .collect();
    let mut discoveries: Vec<&MusicSong> =
        new.into_iter().map(|candidate| &candidate.song).collect();
    random.shuffle(&mut familiar);
    random.shuffle(&mut discoveries);
    let mut mix = MixBuilder::new(DAILY_SONGS_PER_ARTIST);
    let mut position = 0_usize;
    while mix.len() < MIX_LENGTH {
        let wants_familiar = position % (FAMILIAR_IN_A_ROW + 1) < FAMILIAR_IN_A_ROW;
        let (preferred, other) = match wants_familiar {
            true => (&mut familiar, &mut discoveries),
            false => (&mut discoveries, &mut familiar),
        };
        if !mix.take_from(preferred) && !mix.take_from(other) {
            break;
        }
        position += 1;
    }
    mix.songs
}

pub fn compose_weekly(radios: &[Vec<Candidate>], excluded: &HashSet<String>) -> Vec<MusicSong> {
    let mut queues: Vec<Vec<&MusicSong>> = radios
        .iter()
        .map(|radio| {
            radio
                .iter()
                .filter(|candidate| candidate.presence == LibraryPresence::NotInLibrary)
                .map(|candidate| &candidate.song)
                .filter(|song| !excluded.contains(&song.video_id))
                .rev()
                .collect()
        })
        .collect();
    let mut mix = MixBuilder::new(WEEKLY_SONGS_PER_ARTIST);
    while mix.len() < MIX_LENGTH {
        let mut took_any = false;
        for queue in &mut queues {
            if mix.len() < MIX_LENGTH && mix.take_last(queue) {
                took_any = true;
            }
        }
        if !took_any {
            break;
        }
    }
    mix.songs
}

struct MixBuilder {
    songs: Vec<MusicSong>,
    video_ids: HashSet<String>,
    names: HashSet<(String, String)>,
    per_artist: HashMap<String, usize>,
    songs_per_artist: usize,
}

impl MixBuilder {
    fn new(songs_per_artist: usize) -> Self {
        Self {
            songs: Vec::new(),
            video_ids: HashSet::new(),
            names: HashSet::new(),
            per_artist: HashMap::new(),
            songs_per_artist,
        }
    }

    fn len(&self) -> usize {
        self.songs.len()
    }

    fn take_from(&mut self, pool: &mut Vec<&MusicSong>) -> bool {
        pool.retain(|song| self.accepts(song));
        let last_artist = self.songs.last().map(artist_key);
        let pick = pool
            .iter()
            .position(|song| Some(artist_key(song)) != last_artist)
            .or_else(|| (!pool.is_empty()).then_some(0));
        match pick {
            Some(index) => {
                let song = pool.remove(index);
                self.add(song);
                true
            }
            None => false,
        }
    }

    fn take_last(&mut self, queue: &mut Vec<&MusicSong>) -> bool {
        while let Some(song) = queue.pop() {
            if self.accepts(song) {
                self.add(song);
                return true;
            }
        }
        false
    }

    fn accepts(&self, song: &MusicSong) -> bool {
        let artist_count = self
            .per_artist
            .get(&artist_key(song))
            .copied()
            .unwrap_or_default();
        !self.video_ids.contains(&song.video_id)
            && !self.names.contains(&name_key(song))
            && artist_count < self.songs_per_artist
    }

    fn add(&mut self, song: &MusicSong) {
        self.video_ids.insert(song.video_id.clone());
        self.names.insert(name_key(song));
        *self.per_artist.entry(artist_key(song)).or_default() += 1;
        self.songs.push(song.clone());
    }
}

fn artist_key(song: &MusicSong) -> String {
    song.artists
        .first()
        .map(|artist| normalized_name(artist))
        .unwrap_or_default()
}

fn name_key(song: &MusicSong) -> (String, String) {
    (normalized_name(&song.title), artist_key(song))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str, artist: &str) -> MusicSong {
        MusicSong {
            video_id: id.to_owned(),
            title: format!("Title {id}"),
            artists: vec![artist.to_owned()],
            album: None,
            duration_ms: Some(200_000),
            thumbnail_url: None,
        }
    }

    fn candidate(id: &str, artist: &str, presence: LibraryPresence) -> Candidate {
        Candidate {
            song: song(id, artist),
            presence,
        }
    }

    fn many(prefix: &str, count: usize, presence: LibraryPresence) -> Vec<Candidate> {
        (0..count)
            .map(|index| {
                candidate(
                    &format!("{prefix}{index}"),
                    &format!("{prefix} artist {index}"),
                    presence,
                )
            })
            .collect()
    }

    #[test]
    fn daily_mixes_alternate_two_familiar_songs_with_one_new_song() {
        let candidates: Vec<Candidate> = many("lib", 40, LibraryPresence::InLibrary)
            .into_iter()
            .chain(many("new", 40, LibraryPresence::NotInLibrary))
            .collect();
        let mix = compose_daily(&[], &candidates, 7);
        assert_eq!(mix.len(), MIX_LENGTH);
        let pattern: String = mix
            .iter()
            .take(6)
            .map(|song| match song.video_id.starts_with("lib") {
                true => 'F',
                false => 'N',
            })
            .collect();
        assert_eq!(pattern, "FFNFFN");
    }

    #[test]
    fn daily_mixes_are_the_same_for_a_seed_and_differ_between_seeds() {
        let candidates = many("lib", 40, LibraryPresence::InLibrary);
        assert_eq!(
            compose_daily(&[], &candidates, 1),
            compose_daily(&[], &candidates, 1)
        );
        assert_ne!(
            compose_daily(&[], &candidates, 1),
            compose_daily(&[], &candidates, 2)
        );
    }

    #[test]
    fn one_artist_never_fills_a_daily_mix_and_duplicates_are_dropped() {
        let crowded: Vec<Candidate> = (0..20)
            .map(|index| {
                candidate(
                    &format!("x{index}"),
                    "Same Artist",
                    LibraryPresence::InLibrary,
                )
            })
            .chain([candidate(
                "x0",
                "Same Artist",
                LibraryPresence::NotInLibrary,
            )])
            .collect();
        let mix = compose_daily(&[song("seed", "Other")], &crowded, 3);
        let from_same_artist = mix
            .iter()
            .filter(|song| song.artists == ["Same Artist"])
            .count();
        assert_eq!(from_same_artist, DAILY_SONGS_PER_ARTIST);
        assert_eq!(mix.len(), DAILY_SONGS_PER_ARTIST + 1);
    }

    #[test]
    fn when_new_songs_run_out_the_mix_is_filled_with_familiar_ones() {
        let candidates: Vec<Candidate> = many("lib", 10, LibraryPresence::InLibrary)
            .into_iter()
            .chain([candidate("new", "Fresh", LibraryPresence::Unknown)])
            .collect();
        let mix = compose_daily(&[], &candidates, 5);
        assert_eq!(mix.len(), 11);
        assert!(mix.iter().any(|song| song.video_id == "new"));
    }

    #[test]
    fn weekly_mixes_only_hold_songs_known_to_be_new_and_skip_last_week() {
        let radios = vec![
            vec![
                candidate("seed", "Seed Artist", LibraryPresence::InLibrary),
                candidate("a1", "A", LibraryPresence::NotInLibrary),
                candidate("a2", "A", LibraryPresence::NotInLibrary),
                candidate("a3", "A", LibraryPresence::NotInLibrary),
                candidate("unknown", "U", LibraryPresence::Unknown),
            ],
            vec![
                candidate("b1", "B", LibraryPresence::NotInLibrary),
                candidate("old", "C", LibraryPresence::NotInLibrary),
                candidate("a1", "A", LibraryPresence::NotInLibrary),
            ],
        ];
        let excluded: HashSet<String> = ["old".to_owned()].into();
        let ids: Vec<String> = compose_weekly(&radios, &excluded)
            .into_iter()
            .map(|song| song.video_id)
            .collect();
        assert_eq!(ids, ["a1", "b1", "a2"]);
        assert!(compose_weekly(&[], &HashSet::new()).is_empty());
    }
}
