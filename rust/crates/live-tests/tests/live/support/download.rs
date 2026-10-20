use reqwest::header::CONTENT_TYPE;

pub struct Download {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub async fn download(url: &str) -> Download {
    let response = reqwest::get(url).await.expect("request");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = response.bytes().await.expect("body").to_vec();
    assert!(
        status.is_success(),
        "{url} returned {status}: {}",
        String::from_utf8_lossy(&bytes)
    );
    Download {
        content_type,
        bytes,
    }
}

pub fn is_audio(bytes: &[u8]) -> bool {
    let is_mpeg_frame = bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0;
    [b"ID3".as_slice(), b"fLaC", b"OggS", b"RIFF"]
        .iter()
        .any(|magic| bytes.starts_with(magic))
        || is_mpeg_frame
}

pub fn is_image(bytes: &[u8]) -> bool {
    [[0xFF, 0xD8, 0xFF].as_slice(), b"\x89PNG", b"RIFF", b"GIF8"]
        .iter()
        .any(|magic| bytes.starts_with(magic))
}
