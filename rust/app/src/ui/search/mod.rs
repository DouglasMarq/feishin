mod cards;
mod cover_loading;
mod covers;
mod fetch;
mod query;
mod results;
mod searches;
mod searching;
mod section;
mod song_commands;
mod view;

pub use view::install;

#[cfg(test)]
mod test_support {
    use feishin_core::model::Song;

    use crate::demo::test_song;

    pub fn numbered_songs(count: usize) -> Vec<Song> {
        (0..count)
            .map(|number| test_song(&format!("s{number}"), 1000))
            .collect()
    }
}
