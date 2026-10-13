use feishin_storage::credentials;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "touches the real Secret Service keyring"]
async fn real_keyring_inside_tokio_runtime() {
    let id = "feishin-rs-selftest";
    credentials::set(id, "selftest").unwrap();
    assert_eq!(credentials::get(id).unwrap().as_deref(), Some("selftest"));
    credentials::delete(id).unwrap();
    assert_eq!(credentials::get(id).unwrap(), None);
}
