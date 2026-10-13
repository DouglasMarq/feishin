#!/usr/bin/env bash
# Usage: install.sh [--uninstall]
# Builds the release binary and installs it for the current user with an app menu entry.
# Install root defaults to ~/.local; override with PREFIX=/some/dir.
set -euo pipefail

prefix="${PREFIX:-$HOME/.local}"
rust_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
icons_dir="$rust_dir/../assets/icons"
binary="$prefix/bin/feishin"
desktop_file="$prefix/share/applications/feishin.desktop"
icon_sizes=(16 24 32 48 64 128 256 512)

icon_path() {
    echo "$prefix/share/icons/hicolor/${1}x${1}/apps/feishin.png"
}

refresh_menus() {
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" || true
    [[ -f "$prefix/share/icons/hicolor/icon-theme.cache" ]] && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
}

uninstall() {
    rm -f "$binary" "$desktop_file"
    for size in "${icon_sizes[@]}"; do rm -f "$(icon_path "$size")"; done
    refresh_menus
    echo "Removed Feishin from $prefix (settings in ~/.config/feishin-rs were kept)"
}

warn_about_electron_package() {
    command -v pacman >/dev/null && pacman -Q feishin >/dev/null 2>&1 || return 0
    echo "Note: the Electron Feishin package is still installed; remove it with: sudo pacman -Rns feishin" >&2
}

write_desktop_file() {
    install -d "$(dirname "$desktop_file")"
    cat > "$desktop_file" <<EOF
[Desktop Entry]
Type=Application
Name=Feishin
Comment=Self-hosted music player
Exec=$binary
Icon=feishin
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=feishin
EOF
}

install_app() {
    cargo build --release --manifest-path "$rust_dir/Cargo.toml" -p feishin
    install -Dm755 "$rust_dir/target/release/feishin" "$binary"
    for size in "${icon_sizes[@]}"; do
        install -Dm644 "$icons_dir/${size}x${size}.png" "$(icon_path "$size")"
    done
    write_desktop_file
    refresh_menus
    echo "Installed $binary"
    warn_about_electron_package
}

case "${1:-}" in
    "") install_app ;;
    --uninstall) uninstall ;;
    *) echo "usage: install.sh [--uninstall]" >&2; exit 1 ;;
esac
