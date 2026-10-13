#!/usr/bin/env bash
# Usage: install.sh [--uninstall]
# Builds the release binary and installs it for the current user with an app menu entry.
# Install root defaults to ~/.local; override with PREFIX=/some/dir.
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
rust_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
desktop_integration="$rust_dir/packaging/install-desktop-integration.sh"
binary="$prefix/bin/feishin"
legacy_icon_sizes=(16 24 32 48 64 128 256 512)

refresh_menus() {
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" || true
    [[ -f "$prefix/share/icons/hicolor/icon-theme.cache" ]] && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
}

remove_legacy_entry() {
    local legacy_desktop_file="$prefix/share/applications/feishin.desktop"
    grep -qxF "Exec=$binary" "$legacy_desktop_file" 2>/dev/null || return 0
    rm -f "$legacy_desktop_file"
    for size in "${legacy_icon_sizes[@]}"; do
        rm -f "$prefix/share/icons/hicolor/${size}x${size}/apps/feishin.png"
    done
}

uninstall() {
    rm -f "$binary"
    bash "$desktop_integration" --uninstall "$prefix"
    remove_legacy_entry
    refresh_menus
    echo "Removed Feishin from $prefix (settings in ~/.config/feishin-rs were kept)"
}

warn_about_electron_package() {
    command -v pacman >/dev/null && pacman -Q feishin >/dev/null 2>&1 || return 0
    echo "Note: the Electron Feishin package is still installed; remove it with: sudo pacman -Rns feishin" >&2
}

install_app() {
    cargo build --release --manifest-path "$rust_dir/Cargo.toml" -p feishin
    install -Dm755 "$rust_dir/target/release/feishin" "$binary"
    remove_legacy_entry
    bash "$desktop_integration" "$prefix" "$binary"
    refresh_menus
    echo "Installed $binary"
    warn_about_electron_package
}

case "${1:-}" in
    "") install_app ;;
    --uninstall) uninstall ;;
    *) echo "usage: install.sh [--uninstall]" >&2; exit 1 ;;
esac
