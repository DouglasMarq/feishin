use feishin_api::ApiError;

use crate::support::Backend;

pub async fn login_and_ping(backend: Backend) {
    let password = backend.target().password;
    let (server, _) = backend.login_with(&password).await.expect("login");
    server.ping().await.expect("ping");
    assert_eq!(server.config().id, backend.login_test_config().id);
}

pub async fn wrong_password_is_rejected(backend: Backend) {
    let rejected = backend.login_with("definitely-not-the-password").await;
    assert!(
        matches!(rejected, Err(ApiError::Auth(_))),
        "expected an auth error, got {:?}",
        rejected.map(|(_, credential)| credential)
    );
}

pub async fn stored_credential_reconnects(backend: Backend) {
    let password = backend.target().password;
    let (_, credential) = backend.login_with(&password).await.expect("login");
    let restored = backend
        .reconnect(backend.login_test_config(), &credential)
        .expect("reconnect from credential");
    restored
        .ping()
        .await
        .expect("ping with restored credential");
    assert!(
        !restored
            .song_list(&Default::default(), crate::support::everything())
            .await
            .expect("songs with restored credential")
            .items
            .is_empty()
    );
}
