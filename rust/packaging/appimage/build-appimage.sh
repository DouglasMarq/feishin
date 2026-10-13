#!/usr/bin/env bash
# Usage: build-appimage.sh [output-dir]
# Builds Feishin-<version>-<arch>.AppImage with libmpv and its libraries bundled.
# FEISHIN_BINARY=<path> packages an already built binary instead of running cargo.
# LINUXDEPLOY=<path> and APPIMAGETOOL=<path> pick the tools; otherwise they are taken
# from PATH or downloaded into a temporary directory.
set -euo pipefail

app_id="org.jeffvli.feishin.native"
packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rust_dir="$(cd "$packaging_dir/.." && pwd)"
requested_output_dir="${1:-$rust_dir/target/appimage}"
mkdir -p "$requested_output_dir"
output_dir="$(cd "$requested_output_dir" && pwd)"
machine="$(uname -m)"
work_dir="$(mktemp -d -t feishin-appimage.XXXXXX)"
app_dir="$work_dir/AppDir"
trap 'rm -rf "$work_dir"' EXIT
export APPIMAGE_EXTRACT_AND_RUN=1

workspace_version() {
    sed -n 's/^version = "\(.*\)"$/\1/p' "$rust_dir/Cargo.toml" | head -n1
}

release_url() {
    echo "https://github.com/$1/releases/download/continuous/$2-$machine.AppImage"
}

tool_path() {
    local name="$1" override="$2" repository="$3"
    [[ -n "$override" ]] && { echo "$override"; return; }
    command -v "$name" 2>/dev/null && return
    local downloaded="$work_dir/tools/$name"
    mkdir -p "$work_dir/tools"
    echo "Downloading $name" >&2
    curl --fail --location --silent --show-error --output "$downloaded" "$(release_url "$repository" "$name")"
    chmod +x "$downloaded"
    echo "$downloaded"
}

built_binary() {
    [[ -n "${FEISHIN_BINARY:-}" ]] && { echo "$FEISHIN_BINARY"; return; }
    cargo build --release --manifest-path "$rust_dir/Cargo.toml" -p feishin >&2
    echo "$rust_dir/target/release/feishin"
}

assemble_app_dir() {
    install -Dm755 "$(built_binary)" "$app_dir/usr/bin/feishin"
    bash "$packaging_dir/install-desktop-integration.sh" "$app_dir/usr"
    mv "$app_dir/usr/share/metainfo/$app_id.metainfo.xml" "$app_dir/usr/share/metainfo/$app_id.appdata.xml"
}

bundle_libraries() {
    local linuxdeploy
    linuxdeploy="$(tool_path linuxdeploy "${LINUXDEPLOY:-}" linuxdeploy/linuxdeploy)"
    "$linuxdeploy" --appdir "$app_dir" \
        --executable "$app_dir/usr/bin/feishin" \
        --desktop-file "$app_dir/usr/share/applications/$app_id.desktop" \
        --icon-file "$app_dir/usr/share/icons/hicolor/256x256/apps/$app_id.png" \
        --exclude-library 'libxkbcommon*' \
        --exclude-library 'libwayland-*'
}

pack_app_image() {
    local appimagetool target
    appimagetool="$(tool_path appimagetool "${APPIMAGETOOL:-}" AppImage/appimagetool)"
    target="$output_dir/Feishin-$(workspace_version)-$machine.AppImage"
    ARCH="$machine" "$appimagetool" "$app_dir" "$target" >&2
    echo "$target"
}

assemble_app_dir
bundle_libraries
app_image="$(pack_app_image)"
echo "Built $app_image ($(du -h "$app_image" | cut -f1))"
