use std::sync::Arc;

use super::cards::CardKind;
use super::covers::CoverKey;
use super::fetch::fetch;
use super::results::with_results;
use crate::ui::player::to_image;
use crate::ui::{AppWindow, Context, SearchKind};

const SEARCH_COVER_SIZE: u32 = 192;

pub fn load_cover(window: &AppWindow, ctx: &Context, kind: SearchKind, index: i32) {
    let kind = CardKind::from(kind);
    let Ok(index) = usize::try_from(index) else {
        return;
    };
    let claimed = with_results(window, |results| results.claim_cover(kind, index)).flatten();
    let Some((id, cover_id)) = claimed else {
        return;
    };
    let covers = Arc::clone(&ctx.covers);
    let key = CoverKey::new(kind, id.clone());
    let sent = fetch(
        window,
        ctx,
        move |server| async move {
            covers
                .cover(server.as_ref(), &cover_id, SEARCH_COVER_SIZE)
                .await
        },
        move |_, results, result| {
            let image = result
                .inspect_err(|error| tracing::debug!("search cover for {id}: {error:#}"))
                .ok()
                .and_then(|cover| to_image(&cover));
            results.set_cover(kind, index, &id, image);
        },
    );
    if !sent {
        with_results(window, |results| results.covers.borrow_mut().failed(&key));
    }
}
