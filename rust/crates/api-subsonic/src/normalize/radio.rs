use feishin_core::model::RadioStation;

use super::numbers::non_empty_text;
use crate::types::SubsonicRadioStation;

pub fn radio_station(item: SubsonicRadioStation) -> RadioStation {
    RadioStation {
        id: item.id,
        name: item.name,
        stream_url: item.stream_url,
        homepage_url: non_empty_text(item.home_page_url),
    }
}
