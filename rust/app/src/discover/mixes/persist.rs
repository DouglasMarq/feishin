use super::model::{MIX_FORMAT_VERSION, MixSet};
use crate::store::{StateFile, Store};

pub fn load_mixes(store: &dyn Store) -> Option<MixSet> {
    let bytes = store
        .read_state(StateFile::DiscoverMixes)
        .inspect_err(|error| tracing::debug!("reading the saved mixes failed: {error:#}"))
        .ok()??;
    serde_json::from_slice::<MixSet>(&bytes)
        .inspect_err(|error| tracing::debug!("the saved mixes are unreadable: {error}"))
        .ok()
        .filter(|mixes| mixes.version == MIX_FORMAT_VERSION)
}

pub fn save_mixes(store: &dyn Store, mixes: &MixSet) {
    let saved = serde_json::to_vec(mixes)
        .map_err(anyhow::Error::from)
        .and_then(|bytes| store.write_state(StateFile::DiscoverMixes, &bytes));
    if let Err(error) = saved {
        tracing::warn!("saving the mixes failed: {error:#}");
    }
}
