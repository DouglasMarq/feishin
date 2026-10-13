use feishin_storage::credentials;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "touches the real Secret Service keyring"]
async fn real_keyring_inside_tokio_runtime() {
    let server_id = "feishin-rs-selftest";
    credentials::set(server_id, "selftest").unwrap();
    assert_eq!(
        credentials::get(server_id).unwrap().as_deref(),
        Some("selftest")
    );
    credentials::delete(server_id).unwrap();
    assert_eq!(credentials::get(server_id).unwrap(), None);
}
