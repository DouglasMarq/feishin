#[cfg(test)]
mod checks;
#[cfg(test)]
mod support;

#[cfg(test)]
macro_rules! live_checks {
    ($backend:expr; $($name:ident => $check:path),* $(,)?) => {
        $(
            #[tokio::test]
            #[ignore = "needs live servers: rust/scripts/live-servers.sh up"]
            async fn $name() {
                let _server_in_use = $backend.exclusive().await;
                $check($backend).await;
            }
        )*
    };
}

#[cfg(test)]
macro_rules! live_backend {
    ($module:ident, $backend:expr) => {
        mod $module {
            use crate::checks::{
                albums, annotations, artists, edges, filters, library, playback, playlists,
                session, songs, sorting,
            };
            use crate::support::Backend;

            live_checks! {
                $backend;
                login_and_ping => session::login_and_ping,
                wrong_password_is_rejected => session::wrong_password_is_rejected,
                stored_credential_reconnects => session::stored_credential_reconnects,
                song_pages_cover_the_library => songs::pages_cover_the_library,
                songs_sort_by_name_and_duration => songs::sorts_by_name_and_duration,
                songs_filter_by_album_and_artist => songs::filters_by_album_and_artist,
                songs_filter_by_genre_and_text => songs::filters_by_genre_and_text,
                song_details_match_the_tags => songs::song_details_match_the_tags,
                random_and_similar_songs => songs::random_and_similar_songs,
                album_pages_cover_all_albums => albums::pages_cover_all_albums,
                albums_sort_by_name_and_year => albums::sorts_by_name_and_year,
                albums_filter_by_artist_and_genre => albums::filters_by_artist_and_genre,
                album_detail_lists_its_songs => albums::album_detail_lists_its_songs,
                artists_list_sort_and_page => artists::lists_sorts_and_pages_artists,
                artist_detail_lists_albums => artists::artist_detail_lists_albums,
                genres_and_folders => library::lists_genres_and_folders,
                search => library::searches_songs_albums_and_artists,
                library_scan => library::library_scan_starts_and_finishes,
                favorite_songs => annotations::favorites_songs,
                favorite_albums_and_artists => annotations::favorites_albums_and_artists,
                ratings => annotations::rates_songs_and_albums,
                playlists_lifecycle => playlists::creates_edits_and_deletes_playlists,
                scrobbles => playback::scrobbles_update_play_count,
                play_queue => playback::play_queue_round_trips,
                streams => playback::streams_return_audio,
                cover_art => playback::cover_art_downloads,
                lyrics => playback::lyrics_are_found,
                song_media_fields => songs::song_media_fields_are_filled,
                every_song_sort => sorting::every_song_sort_lists_the_library,
                every_album_sort => sorting::every_album_sort_lists_all_albums,
                every_artist_sort => sorting::every_artist_sort_lists_all_artists,
                count_sorts => sorting::count_sorts_order_by_the_counts,
                songs_filter_by_years => filters::songs_filter_by_years,
                songs_filter_by_folder => filters::songs_filter_by_folder,
                albums_filter_by_years_and_folder => filters::albums_filter_by_years_and_folder,
                counts_follow_filters => filters::counts_follow_filters,
                favorite_filters => filters::favorite_filters_list_only_favorites,
                missing_items_are_not_found => edges::missing_items_are_not_found,
                special_characters_round_trip => edges::special_characters_round_trip,
                a_stale_session_token_falls_back => edges::a_stale_session_token_falls_back,
                long_playlists_with_repeats => edges::long_playlists_with_repeats_are_edited,
                opus_streams => edges::streams_transcode_to_opus,
                songs_without_lyrics => edges::songs_without_lyrics_have_none,
                play_queue_can_be_cleared => edges::play_queue_can_be_cleared,
                empty_playlists_and_multi_removal => edges::empty_playlists_and_multi_removal,
                playlist_list_pages => edges::playlist_list_pages,
                search_respects_the_limit => edges::search_respects_the_limit,
                artists_can_be_rated => edges::artists_can_be_rated,
                artist_top_songs_belong_to_the_artist => edges::artist_top_songs_belong_to_the_artist,
            }
        }
    };
}

#[cfg(test)]
live_backend!(navidrome, Backend::Navidrome);
#[cfg(test)]
live_backend!(navidrome_0_54, Backend::Navidrome054);
#[cfg(test)]
live_backend!(navidrome_0_56, Backend::Navidrome056);
#[cfg(test)]
mod navidrome_subsonic {
    use crate::checks::{
        albums, annotations, artists, edges, library, playback, playlists, session, songs,
    };
    use crate::support::Backend;

    live_checks! {
        Backend::NavidromeSubsonic;
        login_and_ping => session::login_and_ping,
        stored_credential_reconnects => session::stored_credential_reconnects,
        song_pages_cover_the_library => songs::pages_cover_the_library,
        songs_list_every_title => songs::sorts_by_name_and_duration,
        song_details_match_the_tags => songs::song_details_match_the_tags,
        song_media_fields => songs::song_media_fields_are_filled,
        random_and_similar_songs => songs::random_and_similar_songs,
        album_pages_cover_all_albums => albums::pages_cover_all_albums,
        albums_sort_by_name_and_year => albums::sorts_by_name_and_year,
        album_detail_lists_its_songs => albums::album_detail_lists_its_songs,
        artists_list_sort_and_page => artists::lists_sorts_and_pages_artists,
        artist_detail_lists_albums => artists::artist_detail_lists_albums,
        genres_and_folders => library::lists_genres_and_folders,
        search => library::searches_songs_albums_and_artists,
        library_scan => library::library_scan_starts_and_finishes,
        favorite_albums_and_artists => annotations::favorites_albums_and_artists,
        ratings => annotations::rates_songs_and_albums,
        playlists_lifecycle => playlists::creates_edits_and_deletes_playlists,
        scrobbles => playback::scrobbles_update_play_count,
        play_queue => playback::play_queue_round_trips,
        streams => playback::streams_return_audio,
        cover_art => playback::cover_art_downloads,
        lyrics => playback::lyrics_are_found,
        missing_items_are_not_found => edges::missing_items_are_not_found,
        special_characters_round_trip => edges::special_characters_round_trip,
        long_playlists_with_repeats => edges::long_playlists_with_repeats_are_edited,
        opus_streams => edges::streams_transcode_to_opus,
        songs_without_lyrics => edges::songs_without_lyrics_have_none,
        play_queue_can_be_cleared => edges::play_queue_can_be_cleared,
        empty_playlists_and_multi_removal => edges::empty_playlists_and_multi_removal,
        playlist_list_pages => edges::playlist_list_pages,
        search_respects_the_limit => edges::search_respects_the_limit,
        artists_can_be_rated => edges::artists_can_be_rated,
        artist_top_songs_belong_to_the_artist => edges::artist_top_songs_belong_to_the_artist,
    }
}
#[cfg(test)]
live_backend!(gonic, Backend::Gonic);
#[cfg(test)]
live_backend!(jellyfin, Backend::Jellyfin);
#[cfg(test)]
live_backend!(jellyfin_10, Backend::Jellyfin10);
