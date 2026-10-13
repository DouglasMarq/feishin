#!/usr/bin/env bash
# Usage: live-servers.sh up|test|down|env
# Runs throwaway Navidrome, Jellyfin and gonic containers on a generated library and runs the live API tests against them.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(dirname "$script_dir")"
state_dir="${FEISHIN_LIVE_STATE_DIR:-$workspace_dir/target/live-servers}"
library_dir="$state_dir/library"
env_file="$state_dir/env"

prefix="feishin-live"
navidrome_port="${FEISHIN_LIVE_NAVIDROME_PORT:-14533}"
gonic_port="${FEISHIN_LIVE_GONIC_PORT:-14747}"
jellyfin_port="${FEISHIN_LIVE_JELLYFIN_PORT:-18096}"
navidrome_image="${FEISHIN_LIVE_NAVIDROME_IMAGE:-docker.io/deluan/navidrome:latest}"
gonic_image="${FEISHIN_LIVE_GONIC_IMAGE:-docker.io/sentriz/gonic:latest}"
jellyfin_image="${FEISHIN_LIVE_JELLYFIN_IMAGE:-docker.io/jellyfin/jellyfin:latest}"

test_user="feishin"
test_password="feishin-live-test"
gonic_user="admin"
gonic_password="admin"
expected_tracks=8

log() { printf '[live-servers] %s\n' "$*" >&2; }
fail() { log "$*"; exit 1; }

require_tools() {
    for tool in podman ffmpeg curl jq cargo; do
        command -v "$tool" >/dev/null || fail "missing tool: $tool"
    done
}

wait_until() {
    local description="$1" attempts="$2"
    shift 2
    for _ in $(seq "$attempts"); do
        if "$@" >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    fail "timed out waiting for $description"
}

tone() {
    local path="$1" frequency="$2" seconds="$3"
    shift 3
    ffmpeg -nostdin -loglevel error -y -f lavfi -i "sine=frequency=$frequency:duration=$seconds" \
        -ac 2 -ar 44100 "$@" "$path"
}

mp3_tone() {
    local path="$1" frequency="$2" seconds="$3"
    shift 3
    tone "$path" "$frequency" "$seconds" -c:a libmp3lame -b:a 128k -id3v2_version 3 "$@"
}

tags() {
    local artist="$1" album="$2" year="$3" genre="$4" track="$5" title="$6"
    printf -- '-metadata\nartist=%s\n-metadata\nalbum_artist=%s\n-metadata\nalbum=%s\n-metadata\ndate=%s\n-metadata\ngenre=%s\n-metadata\ntrack=%s\n-metadata\ntitle=%s\n' \
        "$artist" "$artist" "$album" "$year" "$genre" "$track" "$title"
}

tagged_mp3() {
    local directory="$1" frequency="$2" seconds="$3" artist="$4" album="$5" year="$6" genre="$7" track="$8" title="$9"
    local arguments
    mapfile -t arguments < <(tags "$artist" "$album" "$year" "$genre" "$track" "$title")
    mp3_tone "$directory/$(printf '%02d' "$track") $title.mp3" "$frequency" "$seconds" "${arguments[@]}"
}

synced_lyrics() {
    printf '[00:00.50]Pulling at the shore\n[00:02.00]Tidal line returns\n[00:03.50]Silver undertow\n'
}

lyrics_track() {
    local directory="$1" arguments
    mapfile -t arguments < <(tags "Bravo Sine" "Second Wave" 2021 "Rock" 1 "Tidal Pull")
    tone "$directory/01 Tidal Pull.flac" 523 6 -c:a flac "${arguments[@]}" -metadata "LYRICS=$(synced_lyrics)"
    synced_lyrics >"$directory/01 Tidal Pull.lrc"
}

cover_album() {
    local directory="$library_dir/Aurora Tones/First Light" track title frequency seconds
    mkdir -p "$directory"
    ffmpeg -nostdin -loglevel error -y -f lavfi -i "testsrc=size=600x600:rate=1" -frames:v 1 "$directory/cover.jpg"
    for spec in "1 Dawn Chorus 440 3" "2 Morning Haze 494 4" "3 Noon Glare 554 5"; do
        read -r track title_first title_second frequency seconds <<<"$spec"
        title="$title_first $title_second"
        tagged_mp3 "$directory" "$frequency" "$seconds" "Aurora Tones" "First Light" 2019 "Electronic" "$track" "$title"
    done
}

generate_library() {
    if [[ -f "$library_dir/.complete" ]]; then return 0; fi
    log "generating test library in $library_dir"
    rm -rf "$library_dir"
    cover_album
    mkdir -p "$library_dir/Aurora Tones/Afterglow" "$library_dir/Bravo Sine/Second Wave" "$library_dir/Charlie Square/Third Signal"
    tagged_mp3 "$library_dir/Aurora Tones/Afterglow" 587 7 "Aurora Tones" "Afterglow" 2022 "Electronic" 1 "Dusk Signal"
    lyrics_track "$library_dir/Bravo Sine/Second Wave"
    tagged_mp3 "$library_dir/Bravo Sine/Second Wave" 659 8 "Bravo Sine" "Second Wave" 2021 "Rock" 2 "Undertow"
    tagged_mp3 "$library_dir/Charlie Square/Third Signal" 698 9 "Charlie Square" "Third Signal" 2023 "Jazz" 1 "Blue Note"
    tagged_mp3 "$library_dir/Charlie Square/Third Signal" 784 10 "Charlie Square" "Third Signal" 2023 "Jazz" 2 "Late Set"
    touch "$library_dir/.complete"
}

start_container() {
    local name="$1"
    shift
    podman rm -f "$prefix-$name" >/dev/null 2>&1 || true
    podman run -d --name "$prefix-$name" --label feishin-live=1 "$@" >/dev/null
}

volume() {
    local name="$prefix-$1"
    podman volume exists "$name" || podman volume create --label feishin-live=1 "$name" >/dev/null
    printf '%s' "$name"
}

subsonic_get() {
    local base="$1" user="$2" password="$3" endpoint="$4"
    curl -fsS "$base/rest/$endpoint?u=$user&p=$password&v=1.16.1&c=feishin-live&f=json"
}

subsonic_ok() {
    subsonic_get "$@" | jq -e '."subsonic-response".status == "ok"'
}

scan_settled() {
    local base="$1" user="$2" password="$3"
    subsonic_get "$base" "$user" "$password" "getScanStatus.view" \
        | jq -e --argjson expected "$expected_tracks" \
            '."subsonic-response".scanStatus | .scanning == false and .count >= $expected'
}

navidrome_up() {
    log "starting Navidrome on port $navidrome_port"
    start_container navidrome -p "127.0.0.1:$navidrome_port:4533" \
        -v "$library_dir:/music:ro,z" -v "$(volume navidrome-data):/data" \
        -e ND_LOGLEVEL=warn -e ND_AUTHREQUESTLIMIT=0 -e ND_ENABLEINSIGHTSCOLLECTOR=false -e ND_LASTFM_ENABLED=false \
        -e ND_LISTENBRAINZ_ENABLED=false -e ND_ENABLEEXTERNALSERVICES=false \
        "$navidrome_image"
    local base="http://127.0.0.1:$navidrome_port"
    wait_until "Navidrome HTTP" 90 curl -fsS "$base/app/"
    curl -fs -X POST "$base/auth/createAdmin" -H 'Content-Type: application/json' \
        -d "{\"username\":\"$test_user\",\"password\":\"$test_password\"}" >/dev/null || log "Navidrome admin already exists"
    wait_until "Navidrome scan" 120 scan_settled "$base" "$test_user" "$test_password"
}

gonic_up() {
    log "starting gonic on port $gonic_port"
    start_container gonic -p "127.0.0.1:$gonic_port:80" \
        -v "$library_dir:/music:ro,z" -v "$(volume gonic-data):/data" -v "$(volume gonic-cache):/cache" \
        -v "$(volume gonic-podcasts):/podcasts" -v "$(volume gonic-playlists):/playlists" \
        -e GONIC_SCAN_AT_START_ENABLED=true -e GONIC_SCAN_WATCHER_ENABLED=false \
        "$gonic_image"
    local base="http://127.0.0.1:$gonic_port"
    wait_until "gonic HTTP" 90 subsonic_ok "$base" "$gonic_user" "$gonic_password" "ping.view"
    wait_until "gonic scan" 120 scan_settled "$base" "$gonic_user" "$gonic_password"
}

jellyfin_auth_header() {
    local token="${1:-}"
    printf 'MediaBrowser Client="feishin-live", Device="script", DeviceId="feishin-live-script", Version="1.0.0"'
    if [[ -n "$token" ]]; then printf ', Token="%s"' "$token"; fi
}

jellyfin_post() {
    local base="$1" path="$2" body="$3" token="${4:-}"
    curl -fsS -X POST "$base$path" -H 'Content-Type: application/json' \
        -H "Authorization: $(jellyfin_auth_header "$token")" -d "$body"
}

jellyfin_wizard() {
    local base="$1"
    if curl -fsS "$base/System/Info/Public" | jq -e '.StartupWizardCompleted == true' >/dev/null; then
        return 0
    fi
    log "completing the Jellyfin startup wizard"
    jellyfin_post "$base" "/Startup/Configuration" '{"UICulture":"en-US","MetadataCountryCode":"US","PreferredMetadataLanguage":"en"}' >/dev/null
    curl -fsS "$base/Startup/User" >/dev/null
    jellyfin_post "$base" "/Startup/User" "{\"Name\":\"$test_user\",\"Password\":\"$test_password\"}" >/dev/null
    jellyfin_post "$base" "/Startup/RemoteAccess" '{"EnableRemoteAccess":true,"EnableAutomaticPortMapping":false}' >/dev/null
    jellyfin_post "$base" "/Startup/Complete" '{}' >/dev/null
}

jellyfin_token() {
    local base="$1"
    jellyfin_post "$base" "/Users/AuthenticateByName" "{\"Username\":\"$test_user\",\"Pw\":\"$test_password\"}" | jq -r '.AccessToken'
}

jellyfin_library() {
    local base="$1" token="$2"
    local folders
    folders="$(curl -fsS "$base/Library/VirtualFolders" -H "Authorization: $(jellyfin_auth_header "$token")")"
    if jq -e 'map(select(.Name == "Music")) | length > 0' <<<"$folders" >/dev/null; then return 0; fi
    log "adding the Jellyfin music library"
    jellyfin_post "$base" "/Library/VirtualFolders?name=Music&collectionType=music&refreshLibrary=true" \
        '{"LibraryOptions":{"PathInfos":[{"Path":"/music"}],"EnableRealtimeMonitor":false,"EnableInternetProviders":false,"SaveLocalMetadata":false}}' "$token" >/dev/null
}

jellyfin_scanned() {
    local base="$1" token="$2"
    curl -fsS "$base/Items?IncludeItemTypes=Audio&Recursive=true&Limit=0" -H "Authorization: $(jellyfin_auth_header "$token")" \
        | jq -e --argjson expected "$expected_tracks" '.TotalRecordCount >= $expected'
}

jellyfin_ready() {
    curl -fsS "$1/System/Info/Public" | jq -e '.Id != null'
}

jellyfin_up() {
    log "starting Jellyfin on port $jellyfin_port"
    start_container jellyfin -p "127.0.0.1:$jellyfin_port:8096" \
        -v "$library_dir:/music:ro,z" -v "$(volume jellyfin-config):/config" -v "$(volume jellyfin-cache):/cache" \
        "$jellyfin_image"
    local base="http://127.0.0.1:$jellyfin_port"
    wait_until "Jellyfin HTTP" 180 jellyfin_ready "$base"
    jellyfin_wizard "$base"
    local token
    token="$(jellyfin_token "$base")"
    jellyfin_library "$base" "$token"
    jellyfin_post "$base" "/Library/Refresh" '{}' "$token" >/dev/null
    wait_until "Jellyfin scan" 300 jellyfin_scanned "$base" "$token"
}

write_env() {
    cat >"$env_file" <<EOF
export FEISHIN_LIVE_NAVIDROME_URL=http://127.0.0.1:$navidrome_port
export FEISHIN_LIVE_NAVIDROME_USER=$test_user
export FEISHIN_LIVE_NAVIDROME_PASSWORD=$test_password
export FEISHIN_LIVE_GONIC_URL=http://127.0.0.1:$gonic_port
export FEISHIN_LIVE_GONIC_USER=$gonic_user
export FEISHIN_LIVE_GONIC_PASSWORD=$gonic_password
export FEISHIN_LIVE_JELLYFIN_URL=http://127.0.0.1:$jellyfin_port
export FEISHIN_LIVE_JELLYFIN_USER=$test_user
export FEISHIN_LIVE_JELLYFIN_PASSWORD=$test_password
EOF
    log "server environment written to $env_file"
}

up() {
    require_tools
    mkdir -p "$state_dir"
    generate_library
    navidrome_up
    gonic_up
    jellyfin_up
    write_env
}

run_tests() {
    [[ -f "$env_file" ]] || fail "servers are not up, run: $0 up"
    source "$env_file"
    cd "$workspace_dir"
    cargo test -p feishin-live-tests -- --ignored --test-threads=1 "$@"
}

down() {
    log "removing containers and volumes"
    podman ps -a --filter label=feishin-live=1 --format '{{.Names}}' | xargs -r podman rm -f >/dev/null
    podman volume ls --filter label=feishin-live=1 --format '{{.Name}}' | xargs -r podman volume rm -f >/dev/null
    rm -rf "$state_dir"
}

command="${1:-}"
shift || true
case "$command" in
    up) up ;;
    test) run_tests "$@" ;;
    down) down ;;
    env) cat "$env_file" ;;
    *) fail "usage: $0 up|test|down|env" ;;
esac
