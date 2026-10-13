use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{ItemKind, Song};
use slint::ComponentHandle;

use super::actions::{ActionTarget, SongAction, rate_until_failure};
use super::listed::{LISTED_CHUNK_SIZE, ListedSongs};
use super::with_model;
use crate::controller::Command;
use crate::list_stream::Placement;
use crate::ui::{AppWindow, Context, on_ui, show_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListedEdit {
    Favorite(bool),
    Rate(u8),
}

impl ListedEdit {
    async fn apply(
        self,
        server: Arc<dyn MusicServer>,
        song_ids: Vec<String>,
    ) -> (Vec<String>, Option<String>) {
        match self {
            Self::Favorite(favorite) => {
                match server
                    .set_favorite(ItemKind::Song, &song_ids, favorite)
                    .await
                {
                    Ok(()) => (song_ids, None),
                    Err(error) => (
                        Vec::new(),
                        Some(format!("Could not update favorites: {error}")),
                    ),
                }
            }
            Self::Rate(rating) => {
                let (rated, failure) = rate_until_failure(server, song_ids, rating).await;
                let message = failure.map(|error| format!("Could not set the rating: {error}"));
                (rated, message)
            }
        }
    }

    fn update(self, song: &mut Song) {
        match self {
            Self::Favorite(favorite) => song.favorite = favorite,
            Self::Rate(rating) => song.user_rating = (rating > 0).then_some(rating),
        }
    }
}

pub(super) fn act_on_listed(
    window: &AppWindow,
    target: &ActionTarget,
    action: SongAction,
    listed: ListedSongs,
) {
    match action {
        SongAction::Play => enqueue(&target.ctx, listed, Placement::Now),
        SongAction::PlayNext => enqueue(&target.ctx, listed, Placement::Next),
        SongAction::AddLast => enqueue(&target.ctx, listed, Placement::Last),
        SongAction::AddToPlaylist => target.picker.open_listed(window, listed),
        SongAction::Favorite(favorite) => {
            spawn_edit(window, &target.ctx, listed, ListedEdit::Favorite(favorite));
        }
        SongAction::Rate(rating) => {
            spawn_edit(window, &target.ctx, listed, ListedEdit::Rate(rating))
        }
    }
}

fn enqueue(ctx: &Context, listed: ListedSongs, placement: Placement) {
    ctx.send(Command::EnqueueFromList {
        source: listed.source,
        ranges: listed.ranges.into_ranges(),
        placement,
    });
}

fn spawn_edit(window: &AppWindow, ctx: &Context, listed: ListedSongs, edit: ListedEdit) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        if let Some(message) = edit_listed(server, &weak, listed, edit).await {
            on_ui(&weak, move |window| show_status(window, &message));
        }
    });
}

async fn edit_listed(
    server: Arc<dyn MusicServer>,
    weak: &slint::Weak<AppWindow>,
    listed: ListedSongs,
    edit: ListedEdit,
) -> Option<String> {
    let mut chunks = listed.into_chunks(LISTED_CHUNK_SIZE);
    loop {
        let song_ids = match chunks.next_ids(server.as_ref()).await {
            Ok(Some(song_ids)) => song_ids,
            Ok(None) => return None,
            Err(error) => return Some(format!("Could not load the selection: {error}")),
        };
        let (edited, failure) = edit.apply(Arc::clone(&server), song_ids).await;
        show_edited(weak, edit, edited);
        if failure.is_some() {
            return failure;
        }
    }
}

fn show_edited(weak: &slint::Weak<AppWindow>, edit: ListedEdit, edited: Vec<String>) {
    on_ui(weak, move |window| {
        with_model(window, |model| {
            model.update_songs(&edited, |song| edit.update(song));
        });
    });
}
