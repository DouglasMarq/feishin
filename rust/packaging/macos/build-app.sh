#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rust_dir="$(cd "$script_dir/../.." && pwd)"
repo_dir="$(cd "$rust_dir/.." && pwd)"
out_dir="${OUT_DIR:-$rust_dir/target/macos}"
app="$out_dir/Feishin.app"
contents="$app/Contents"

for tool in cargo dylibbundler iconutil sips codesign ditto; do
    command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 1; }
done

version="$(cargo metadata --manifest-path "$rust_dir/Cargo.toml" --no-deps --format-version 1 \
    | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "feishin"))')"
arch="$(uname -m)"

cargo build --release --manifest-path "$rust_dir/Cargo.toml" -p feishin

rm -rf "$app"
mkdir -p "$contents/MacOS" "$contents/Resources" "$contents/Frameworks"
cp "$rust_dir/target/release/feishin" "$contents/MacOS/feishin"
sed "s/@VERSION@/$version/g" "$script_dir/Info.plist" > "$contents/Info.plist"

iconset="$(mktemp -d)/Feishin.iconset"
mkdir -p "$iconset"
source_icon="$repo_dir/assets/icons/512x512.png"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$source_icon" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    if [ "$double" -le 512 ]; then
        sips -z "$double" "$double" "$source_icon" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    fi
done
iconutil -c icns "$iconset" -o "$contents/Resources/Feishin.icns"

dylibbundler --overwrite-dir --bundle-deps --create-dir \
    --fix-file "$contents/MacOS/feishin" \
    --dest-dir "$contents/Frameworks" \
    --install-path @executable_path/../Frameworks/ >/dev/null

drop_duplicate_rpaths() {
    local binary="$1"
    otool -l "$binary" | awk '/LC_RPATH/ { getline; getline; print $2 }' | sort | uniq -d \
        | while read -r rpath; do
            while [ "$(otool -l "$binary" | awk '/LC_RPATH/ { getline; getline; print $2 }' | grep -cxF "$rpath")" -gt 1 ]; do
                install_name_tool -delete_rpath "$rpath" "$binary"
            done
        done
}

for binary in "$contents/MacOS/feishin" "$contents/Frameworks/"*.dylib; do
    drop_duplicate_rpaths "$binary"
done

codesign --force --deep --sign - "$app"

archive="$out_dir/Feishin-$version-macos-$arch.zip"
rm -f "$archive"
ditto -c -k --keepParent "$app" "$archive"
echo "$app"
echo "$archive"
