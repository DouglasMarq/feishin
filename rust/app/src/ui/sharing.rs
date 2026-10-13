use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{ServerType, Share};
use slint::ComponentHandle;

use super::{AppWindow, Context, ShareKind, ShareState, on_ui, show_status, tracks};

const NOT_AUTHORIZED_CODE: i32 = 50;
const SELECTION_NOT_LOADED: &str = "Scroll through the selected tracks first, then share them";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShareRequest {
    item_ids: Vec<String>,
    description: Option<String>,
}

impl ShareRequest {
    fn single(id: &str, name: &str) -> Self {
        Self {
            item_ids: vec![id.to_owned()],
            description: Some(name.trim().to_owned()).filter(|name| !name.is_empty()),
        }
    }
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<ShareState>();

    let (weak, share_ctx) = (window.as_weak(), ctx.clone());
    state.on_share(move |_kind: ShareKind, id, name| {
        if let Some(window) = weak.upgrade() {
            start_share(&window, &share_ctx, ShareRequest::single(&id, &name));
        }
    });

    let (weak, tracks_ctx) = (window.as_weak(), ctx.clone());
    state.on_share_selected_tracks(move || {
        if let Some(window) = weak.upgrade() {
            share_selected_tracks(&window, &tracks_ctx);
        }
    });
}

pub fn show_availability(window: &AppWindow, server: &dyn MusicServer) {
    let state = window.global::<ShareState>();
    state.set_available(server.can_share());
    state.set_unavailable_reason(unavailable_reason(server.config().server_type).into());
}

fn unavailable_reason(server_type: ServerType) -> &'static str {
    match server_type {
        ServerType::Jellyfin => "Jellyfin has no public share links",
        ServerType::Navidrome | ServerType::Subsonic => "this server has no public share links",
    }
}

fn share_selected_tracks(window: &AppWindow, ctx: &Context) {
    match tracks::selected_song_ids(window) {
        Some(item_ids) if item_ids.is_empty() => {}
        Some(item_ids) => start_share(
            window,
            ctx,
            ShareRequest {
                item_ids,
                description: None,
            },
        ),
        None => show_status(window, SELECTION_NOT_LOADED),
    }
}

fn start_share(window: &AppWindow, ctx: &Context, request: ShareRequest) {
    let state = window.global::<ShareState>();
    if state.get_busy() {
        return;
    }
    let Some(server) = ctx.session.get() else {
        show_status(window, "Not connected to a server");
        return;
    };
    if !server.can_share() {
        let reason = unavailable_reason(server.config().server_type);
        show_status(window, &format!("Cannot share: {reason}"));
        return;
    }
    state.set_busy(true);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let description = request.description.as_deref();
        let result = server.create_share(&request.item_ids, description).await;
        on_ui(&weak, move |window| finish_share(window, result));
    });
}

fn finish_share(window: &AppWindow, result: Result<Share, ApiError>) {
    window.global::<ShareState>().set_busy(false);
    match result {
        Ok(share) => {
            copy_link(window, &share.url);
            show_status(
                window,
                &format!("Share link copied to the clipboard: {}", share.url),
            );
        }
        Err(error) => show_status(window, &failure_message(&error)),
    }
}

fn copy_link(window: &AppWindow, url: &str) {
    let state = window.global::<ShareState>();
    state.set_link(url.into());
    state.set_copy_request(state.get_copy_request().wrapping_add(1));
}

fn failure_message(error: &ApiError) -> String {
    match error {
        ApiError::Server {
            code: NOT_AUTHORIZED_CODE,
            ..
        } => "Your user is not allowed to share on this server".into(),
        ApiError::Unsupported(reason) => format!("Could not create a share link: {reason}"),
        other => format!("Could not create a share link: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_requests_describe_the_item_by_name() {
        let request = ShareRequest::single("album-1", " Blue Train ");
        assert_eq!(request.item_ids, ["album-1"]);
        assert_eq!(request.description.as_deref(), Some("Blue Train"));
        assert_eq!(ShareRequest::single("song-1", "  ").description, None);
    }

    #[test]
    fn failures_explain_what_went_wrong() {
        let disabled = ApiError::Unsupported("sharing is turned off on this server");
        assert_eq!(
            failure_message(&disabled),
            "Could not create a share link: sharing is turned off on this server"
        );
        let refused = ApiError::Server {
            code: NOT_AUTHORIZED_CODE,
            message: "no share role".into(),
        };
        assert_eq!(
            failure_message(&refused),
            "Your user is not allowed to share on this server"
        );
        let offline = ApiError::Network("timed out".into());
        assert_eq!(
            failure_message(&offline),
            "Could not create a share link: network error: timed out"
        );
    }

    #[test]
    fn jellyfin_explains_why_sharing_is_off() {
        assert_eq!(
            unavailable_reason(ServerType::Jellyfin),
            "Jellyfin has no public share links"
        );
    }
}
