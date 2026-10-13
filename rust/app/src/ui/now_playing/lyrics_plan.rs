use feishin_lyrics::LyricsSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LyricsStep {
    Server,
    Remote(Vec<LyricsSource>),
}

enum ConfiguredSource {
    Server,
    Remote(LyricsSource),
    Unknown,
}

fn parse_source(name: &str) -> ConfiguredSource {
    match name.trim().to_ascii_lowercase().as_str() {
        "server" => ConfiguredSource::Server,
        "lrclib" => ConfiguredSource::Remote(LyricsSource::Lrclib),
        "netease" => ConfiguredSource::Remote(LyricsSource::NetEase),
        "genius" => ConfiguredSource::Remote(LyricsSource::Genius),
        other => {
            tracing::debug!("unknown lyrics source {other}");
            ConfiguredSource::Unknown
        }
    }
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

pub(super) fn lyrics_plan(names: &[String]) -> Vec<LyricsStep> {
    let mut plan: Vec<LyricsStep> = Vec::new();
    for name in names {
        match parse_source(name) {
            ConfiguredSource::Server => add_server_step(&mut plan),
            ConfiguredSource::Remote(source) => add_remote_source(&mut plan, source),
            ConfiguredSource::Unknown => {}
        }
    }
    plan
}

#[cfg(test)]
pub(super) fn source_names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_follows_settings_order_and_groups_remote_sources() {
        assert_eq!(
            lyrics_plan(&source_names(&[
                "server", "lrclib", "NetEase", "bogus", "lrclib"
            ])),
            vec![
                LyricsStep::Server,
                LyricsStep::Remote(vec![LyricsSource::Lrclib, LyricsSource::NetEase])
            ]
        );
        assert_eq!(
            lyrics_plan(&source_names(&["genius", "server", "lrclib"])),
            vec![
                LyricsStep::Remote(vec![LyricsSource::Genius]),
                LyricsStep::Server,
                LyricsStep::Remote(vec![LyricsSource::Lrclib]),
            ]
        );
        assert!(lyrics_plan(&[]).is_empty());
    }
}
