use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, SearchResults};

use super::fetch::{fetch, report};
use super::query::{SEARCH_LIMIT, album_query, artist_query, song_query};
use super::results::{Results, with_results};
use super::searches::{More, MoreRequest};
use crate::ui::{AppWindow, Context, SearchKind};

pub fn search(window: &AppWindow, ctx: &Context, query: Option<String>, force: bool) {
    let started = with_results(window, |results| {
        results.begin_search(window, query.clone(), force)
    })
    .flatten();
    let (Some(generation), Some(query)) = (started, query) else {
        return;
    };
    let sent = fetch(
        window,
        ctx,
        move |server| async move { server.search(&query, SEARCH_LIMIT).await },
        move |window, results, result| finish_search(window, results, generation, result),
    );
    if !sent {
        with_results(window, |results| {
            abandon_search(window, results, generation)
        });
    }
}

fn finish_search(
    window: &AppWindow,
    results: &Results,
    generation: u64,
    result: feishin_api::Result<SearchResults>,
) {
    let found = result
        .inspect_err(|error| report(window, "Search failed", error))
        .ok();
    let applied = results
        .searches
        .borrow_mut()
        .finish(generation, found, SEARCH_LIMIT);
    if applied {
        results.show_all(window);
    }
}

fn abandon_search(window: &AppWindow, results: &Results, generation: u64) {
    results
        .searches
        .borrow_mut()
        .finish(generation, None, SEARCH_LIMIT);
    results.show_flags(window);
}

pub fn show_more(window: &AppWindow, ctx: &Context, kind: SearchKind) {
    let requested = with_results(window, |results| {
        let request = results.searches.borrow_mut().more(kind);
        results.show_flags(window);
        request
    })
    .flatten();
    let Some(MoreRequest {
        generation,
        query,
        paging,
    }) = requested
    else {
        return;
    };
    let sent = fetch(
        window,
        ctx,
        move |server| fetch_more(server, kind, query, paging),
        move |window, results, result| {
            apply_more(window, results, kind, generation, paging, result);
        },
    );
    if !sent {
        with_results(window, |results| {
            abandon_more(window, results, generation, kind)
        });
    }
}

async fn fetch_more(
    server: Arc<dyn MusicServer>,
    kind: SearchKind,
    query: String,
    paging: Paging,
) -> feishin_api::Result<More> {
    match kind {
        SearchKind::Songs => server
            .song_list(&song_query(&query), paging)
            .await
            .map(|page| More::Songs(page.items)),
        SearchKind::Albums => server
            .album_list(&album_query(&query), paging)
            .await
            .map(|page| More::Albums(page.items)),
        SearchKind::Artists => server
            .artist_list(&artist_query(&query), paging)
            .await
            .map(|page| More::Artists(page.items)),
    }
}

fn apply_more(
    window: &AppWindow,
    results: &Results,
    kind: SearchKind,
    generation: u64,
    paging: Paging,
    result: feishin_api::Result<More>,
) {
    let applied = match result {
        Ok(more) => results
            .searches
            .borrow_mut()
            .apply_more(generation, paging, more),
        Err(error) => {
            results.searches.borrow_mut().more_failed(generation, kind);
            report(window, "Could not load more results", error);
            None
        }
    };
    match applied {
        Some(applied) => results.show_more(window, kind, &applied),
        None => results.show_flags(window),
    }
}

fn abandon_more(window: &AppWindow, results: &Results, generation: u64, kind: SearchKind) {
    results.searches.borrow_mut().more_failed(generation, kind);
    results.show_flags(window);
}
