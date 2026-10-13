#!/usr/bin/env bash
# Usage: install-desktop-integration.sh <prefix> [exec-command]
# Installs the shared desktop entry, AppStream metainfo and hicolor icons of the
# native app under <prefix> (for example "$pkgdir/usr", "AppDir/usr" or ~/.local).
# exec-command replaces the Exec= line of the desktop entry (default: feishin).
# Run with --uninstall <prefix> to remove those files again.
set -euo pipefail

app_id="org.jeffvli.feishin.native"
packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
icons_dir="$packaging_dir/../../assets/icons"
icon_sizes=(16 24 32 48 64 72 96 128 256 512)

desktop_path() { echo "$1/share/applications/$app_id.desktop"; }
metainfo_path() { echo "$1/share/metainfo/$app_id.metainfo.xml"; }
icon_path() { echo "$1/share/icons/hicolor/${2}x${2}/apps/$app_id.png"; }

install_files() {
    local prefix="$1" exec_command="$2"
    install -d "$(dirname "$(desktop_path "$prefix")")"
    sed "s|^Exec=feishin\$|Exec=$exec_command|" "$packaging_dir/$app_id.desktop" > "$(desktop_path "$prefix")"
    chmod 644 "$(desktop_path "$prefix")"
    install -Dm644 "$packaging_dir/$app_id.metainfo.xml" "$(metainfo_path "$prefix")"
    for size in "${icon_sizes[@]}"; do
        install -Dm644 "$icons_dir/${size}x${size}.png" "$(icon_path "$prefix" "$size")"
    done
}

remove_files() {
    local prefix="$1"
    rm -f "$(desktop_path "$prefix")" "$(metainfo_path "$prefix")"
    for size in "${icon_sizes[@]}"; do rm -f "$(icon_path "$prefix" "$size")"; done
}

case "${1:-}" in
    "" | -h | --help) echo "usage: install-desktop-integration.sh [--uninstall] <prefix> [exec-command]" >&2; exit 1 ;;
    --uninstall) remove_files "${2:?missing prefix}" ;;
    *) install_files "$1" "${2:-feishin}" ;;
esac
