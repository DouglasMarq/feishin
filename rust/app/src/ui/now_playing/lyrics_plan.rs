use feishin_lyrics::LyricsSource;
use feishin_storage::settings::LyricsSourceSetting;

use crate::lyrics_provider::remote_lyrics_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LyricsStep {
    Server,
    Remote(Vec<LyricsSource>),
}

fn add_server_step(plan: &mut Vec<LyricsStep>) {
    if !plan.contains(&LyricsStep::Server) {
        plan.push(LyricsStep::Server);
    }
}

fn add_remote_source(plan: &mut Vec<LyricsStep>, source: LyricsSource) {
    let already_planned = plan
        .iter()
        .any(|step| matches!(step, LyricsStep::Remote(sources) if sources.contains(&source)));
    if already_planned {
        return;
    }
    match plan.last_mut() {
        Some(LyricsStep::Remote(sources)) => sources.push(source),
        _ => plan.push(LyricsStep::Remote(vec![source])),
    }
}

pub(super) fn lyrics_plan(configured: &[LyricsSourceSetting]) -> Vec<LyricsStep> {
    let mut plan: Vec<LyricsStep> = Vec::new();
    for &source in configured {
        match remote_lyrics_source(source) {
            None => add_server_step(&mut plan),
            Some(remote) => add_remote_source(&mut plan, remote),
        }
    }
    plan
}

pub(super) fn remote_sources(configured: &[LyricsSourceSetting]) -> Vec<LyricsSource> {
    lyrics_plan(configured)
        .into_iter()
        .flat_map(|step| match step {
            LyricsStep::Server => Vec::new(),
            LyricsStep::Remote(sources) => sources,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_follows_settings_order_and_groups_remote_sources() {
        assert_eq!(
            lyrics_plan(&[
                LyricsSourceSetting::Server,
                LyricsSourceSetting::Lrclib,
                LyricsSourceSetting::NetEase,
                LyricsSourceSetting::Lrclib
            ]),
            vec![
                LyricsStep::Server,
                LyricsStep::Remote(vec![LyricsSource::Lrclib, LyricsSource::NetEase])
            ]
        );
        assert_eq!(
            lyrics_plan(&[
                LyricsSourceSetting::Genius,
                LyricsSourceSetting::Server,
                LyricsSourceSetting::Lrclib
            ]),
            vec![
                LyricsStep::Remote(vec![LyricsSource::Genius]),
                LyricsStep::Server,
                LyricsStep::Remote(vec![LyricsSource::Lrclib]),
            ]
        );
        assert!(lyrics_plan(&[]).is_empty());
    }

    #[test]
    fn remote_sources_skip_the_server() {
        assert_eq!(
            remote_sources(&[
                LyricsSourceSetting::Server,
                LyricsSourceSetting::Genius,
                LyricsSourceSetting::Lrclib
            ]),
            vec![LyricsSource::Genius, LyricsSource::Lrclib]
        );
        assert!(remote_sources(&[LyricsSourceSetting::Server]).is_empty());
    }
}
