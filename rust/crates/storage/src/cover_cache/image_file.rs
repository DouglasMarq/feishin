use std::path::Path;
use std::time::SystemTime;

use image::DynamicImage;
use image::imageops::FilterType;
use sha2::{Digest, Sha256};

use super::DecodedImage;
use crate::{Result, StorageError};

pub(super) fn decode(bytes: &[u8], size: u32) -> Result<DecodedImage> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| StorageError::Invalid(format!("cover image: {error}")))?;
    let rgba = shrink_to_fit(image, size).into_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

fn shrink_to_fit(image: DynamicImage, size: u32) -> DynamicImage {
    let is_larger_than_size = image.width() > size || image.height() > size;
    match size > 0 && is_larger_than_size {
        true => image.resize(size, size, FilterType::CatmullRom),
        false => image,
    }
}

pub(super) fn cache_file_name(key: &str, size: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.update([0]);
    hasher.update(size.to_le_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn mark_recently_used(path: &Path) {
    let refreshed = std::fs::File::options()
        .append(true)
        .open(path)
        .and_then(|file| file.set_modified(SystemTime::now()));
    if let Err(error) = refreshed {
        tracing::debug!("could not refresh cover cache file time for {path:?}: {error}");
    }
}
