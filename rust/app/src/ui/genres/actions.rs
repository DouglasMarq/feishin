use feishin_core::model::ItemKind;
use slint::ComponentHandle;

use super::binding::{Binding, with_grid, with_table};
use crate::controller::Command;
use crate::list_stream::{ListRow, SongSource};
use crate::ui::{AppWindow, CardRow, Context, TrackItem, on_ui, show_status};

pub fn toggle_card(window: &AppWindow, ctx: &Context, binding: Binding<CardRow>, id: &str) {
    let flipped = with_grid(window, binding, |grid| {
        let favorite = grid.favorite(id)?;
        grid.set_favorite(id, !favorite);
        Some((grid.kind()?, favorite))
    })
    .flatten();
    let Some((kind, favorite)) = flipped else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let id = id.to_owned();
    ctx.rt.spawn(async move {
        if let Err(error) = server
            .set_favorite(kind, std::slice::from_ref(&id), !favorite)
            .await
        {
            on_ui(&weak, move |window| {
                with_grid(window, binding, |grid| grid.set_favorite(&id, favorite));
                show_status(window, &format!("Could not update favorite: {error}"));
            });
        }
    });
}

pub fn toggle_track(window: &AppWindow, ctx: &Context, binding: Binding<TrackItem>, index: usize) {
    let flipped = with_table(window, binding, |table| {
        let (song_id, favorite) = table.favorite_at(index)?;
        table.set_favorite(index, &song_id, !favorite);
        Some((song_id, favorite))
    })
    .flatten();
    let Some((song_id, favorite)) = flipped else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        if let Err(error) = server
            .set_favorite(ItemKind::Song, std::slice::from_ref(&song_id), !favorite)
            .await
        {
            on_ui(&weak, move |window| {
                with_table(window, binding, |table| {
                    table.set_favorite(index, &song_id, favorite);
                });
                show_status(window, &format!("Could not update favorite: {error}"));
            });
        }
    });
}

pub fn play_track(window: &AppWindow, ctx: &Context, binding: Binding<TrackItem>, index: i32) {
    let Ok(index) = usize::try_from(index) else {
        return;
    };
    let target = with_table(window, binding, |table| {
        (table.query(), table.song_id_at(index))
    });
    if let Some((query, song_id)) = target {
        ctx.send(Command::PlayFromList {
            source: SongSource::Library(query),
            song_id,
            row_index: ListRow(index),
        });
    }
}
