#!/usr/bin/env bash
# Usage: build-appimage.sh [output-dir]
# Builds Feishin-<version>-<arch>.AppImage with libmpv and its libraries bundled.
# FEISHIN_BINARY=<path> packages an already built binary instead of running cargo.
# LINUXDEPLOY=<path>, APPIMAGETOOL=<path> and APPIMAGE_RUNTIME=<path> pick the tools;
# otherwise pinned releases are downloaded into a temporary directory and their
# SHA-256 checksums verified.
set -euo pipefail
shopt -s inherit_errexit

app_id="org.jeffvli.feishin.native"
packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rust_dir="$(cd "$packaging_dir/.." && pwd)"
requested_output_dir="${1:-$rust_dir/target/appimage}"
mkdir -p "$requested_output_dir"
output_dir="$(cd "$requested_output_dir" && pwd)"
machine="$(uname -m)"
work_dir="$(mktemp -d -t feishin-appimage.XXXXXX)"
app_dir="$work_dir/AppDir"
forced_libraries=(libjack.so.0 libpipewire-0.3.so.0)
trap 'rm -rf "$work_dir"' EXIT
export APPIMAGE_EXTRACT_AND_RUN=1

workspace_version() {
    sed -n 's/^version = "\(.*\)"$/\1/p' "$rust_dir/Cargo.toml" | head -n1
}

declare -A tool_urls=(
    [linuxdeploy]="https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-$machine.AppImage"
    [appimagetool]="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-$machine.AppImage"
    [runtime]="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-$machine"
)
declare -A tool_checksums=(
    [linuxdeploy-x86_64]=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
    [linuxdeploy-aarch64]=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
    [appimagetool-x86_64]=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
    [appimagetool-aarch64]=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
    [runtime-x86_64]=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
    [runtime-aarch64]=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
)

tool_path() {
    local name="$1" override="$2"
    [[ -n "$override" ]] && { echo "$override"; return; }
    local checksum="${tool_checksums[$name-$machine]:-}"
    [[ -n "$checksum" ]] || { echo "no pinned $name for $machine; set its path variable" >&2; exit 1; }
    local downloaded="$work_dir/tools/$name"
    mkdir -p "$work_dir/tools"
    echo "Downloading $name" >&2
    curl --fail --location --silent --show-error --proto '=https' --output "$downloaded" "${tool_urls[$name]}"
    echo "$checksum  $downloaded" | sha256sum --check --quiet >&2
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

forced_library_arguments() {
    local soname library_path
    for soname in "${forced_libraries[@]}"; do
        library_path="$(ldd "$app_dir/usr/bin/feishin" | awk -v soname="$soname" '$1 == soname && $3 ~ /^\// { print $3 }')"
        [[ -n "$library_path" ]] && printf '%s\n' --library "$library_path"
    done
}

bundle_libraries() {
    local linuxdeploy forced_arguments
    linuxdeploy="$(tool_path linuxdeploy "${LINUXDEPLOY:-}")"
    mapfile -t forced_arguments < <(forced_library_arguments)
    "$linuxdeploy" --appdir "$app_dir" "${forced_arguments[@]}" \
        --executable "$app_dir/usr/bin/feishin" \
        --desktop-file "$app_dir/usr/share/applications/$app_id.desktop" \
        --icon-file "$app_dir/usr/share/icons/hicolor/256x256/apps/$app_id.png" \
        --exclude-library 'libxkbcommon*' \
        --exclude-library 'libwayland-*'
}

pack_app_image() {
    local appimagetool runtime target
    appimagetool="$(tool_path appimagetool "${APPIMAGETOOL:-}")"
    runtime="$(tool_path runtime "${APPIMAGE_RUNTIME:-}")"
    target="$output_dir/Feishin-$(workspace_version)-$machine.AppImage"
    ARCH="$machine" "$appimagetool" --runtime-file "$runtime" "$app_dir" "$target" >&2
    echo "$target"
}

assemble_app_dir
bundle_libraries
app_image="$(pack_app_image)"
echo "Built $app_image ($(du -h "$app_image" | cut -f1))"
