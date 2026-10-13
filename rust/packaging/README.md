# Packaging the native app

Everything needed to ship the Rust rewrite of Feishin on Linux. The Electron app and its
packaging are untouched; the native app is a separate package that conflicts with it because
both install a `feishin` binary.

| Path | What it is |
| --- | --- |
| `org.jeffvli.feishin.native.desktop` | Desktop entry shared by every package (`Exec=feishin`) |
| `org.jeffvli.feishin.native.metainfo.xml` | AppStream metainfo for software centers |
| `install-desktop-integration.sh` | Installs the desktop entry, metainfo and hicolor icons under a prefix |
| `arch/PKGBUILD` | `feishin-native-git` package for Arch Linux |
| `appimage/build-appimage.sh` | Builds a portable AppImage with libmpv bundled |

The app id is `org.jeffvli.feishin.native`. The binary sets it as the Wayland app id and X11
`WM_CLASS`, and reports it as the MPRIS `DesktopEntry`, so the window, the media controls and
the menu entry are matched to the same icon. The icon name is the app id too, so the native
app can be installed next to the Electron app's icons without clashing.

## Install for the current user

```sh
rust/scripts/install.sh              # builds and installs into ~/.local
PREFIX=/opt/feishin rust/scripts/install.sh
rust/scripts/install.sh --uninstall
```

It reuses `install-desktop-integration.sh`, pointing `Exec=` at the installed binary, and
removes the `feishin.desktop` entry written by earlier versions of the script.

## Arch Linux package

```sh
cd rust/packaging/arch
makepkg -si
```

The package builds the workspace from `https://github.com/DouglasMarq/feishin`, branch
`feat/rust-native`, installs `/usr/bin/feishin`, the desktop entry, the metainfo and the icons
from `assets/icons`, depends on `mpv` and the libraries winit opens at runtime (Wayland, X11,
xkbcommon, EGL) plus any `ttf-font`, and conflicts with `feishin`, `feishin-bin`, `feishin-git` and
`feishin-appimage`.

### Building from a local commit

`FEISHIN_SOURCE` replaces the source URL, which is how the package is tested before a commit
is pushed. makepkg clones with `git clone --mirror`, so point it at a bare clone (a git
worktree directory cannot be mounted into a container on its own):

```sh
git clone --bare --single-branch -b "$(git branch --show-current)" . /tmp/feishin.git
commit="$(git rev-parse HEAD)"
FEISHIN_SOURCE="git+file:///tmp/feishin.git#commit=$commit" makepkg
```

To build in a clean Arch container as a non-root user (`rust/packaging/arch` has no other
files, so the PKGBUILD is read straight from the commit):

```sh
podman run --rm -it -e commit="$commit" -v /tmp/feishin.git:/src/feishin.git:ro,Z \
    docker.io/library/archlinux bash
pacman -Syu --noconfirm --needed base-devel git
useradd -m builder
su builder -c "git config --global --add safe.directory '*'"
su builder -c "git --git-dir=/src/feishin.git show $commit:rust/packaging/arch/PKGBUILD > ~/PKGBUILD"
pacman -S --noconfirm --needed $(source ~builder/PKGBUILD && echo "${depends[@]} ${makedepends[@]}")
su builder -c "cd ~ && FEISHIN_SOURCE='git+file:///src/feishin.git#commit=$commit' makepkg"
pacman -U --noconfirm ~builder/feishin-native-git-*.pkg.tar.zst
```

## AppImage

```sh
rust/packaging/appimage/build-appimage.sh            # writes rust/target/appimage/Feishin-<version>-x86_64.AppImage
rust/packaging/appimage/build-appimage.sh ~/out      # another output directory
```

The script builds the release binary, installs it and the desktop integration into an
AppDir, lets `linuxdeploy` bundle `libmpv` and every library it links (ffmpeg, libass,
libplacebo, PipeWire, PulseAudio, ...) and packs the result with `appimagetool` and the AppImage
type 2 runtime. Pinned releases of the three are downloaded into a temporary directory that is
removed afterwards, and the build stops if a download does not match its SHA-256 checksum. The
tools run with `APPIMAGE_EXTRACT_AND_RUN=1`, so FUSE is not needed while building.

| Variable | Effect |
| --- | --- |
| `FEISHIN_BINARY` | Package this binary instead of running `cargo build` |
| `LINUXDEPLOY` | Path to a linuxdeploy executable |
| `APPIMAGETOOL` | Path to an appimagetool executable |
| `APPIMAGE_RUNTIME` | Path to an AppImage type 2 runtime file |

What is bundled and what is not:

- `libmpv` and everything it links (ffmpeg, libass, libplacebo, libpulse, ...) are bundled.
- `libjack` and `libpipewire` are on the AppImage exclude list but are bundled anyway: libmpv
  links them, so a system without JACK or PipeWire could not start the AppImage otherwise. The
  bundled PipeWire client only finds its plugins on Debian/Ubuntu library layouts; elsewhere it
  prints a `pw.loop ... can't make support.system handle` line and mpv falls back to the
  PulseAudio protocol, which `pipewire-pulse` serves.
- Taken from the system: glibc, `libstdc++`, `libgcc_s`, GL/EGL drivers, X11/xcb, ALSA,
  fontconfig, freetype and the rest of the default exclude list, plus `libxkbcommon` and the
  `libwayland-*` libraries. winit opens `libxkbcommon-x11` from the system with `dlopen`, and it
  must match `libxkbcommon` exactly (a bundled copy crashed on exit).
- At least one system font is required; every desktop has one.

An AppImage only runs on systems whose glibc is at least as new as the one it was built
against, so build release AppImages on an old, stable base rather than on a rolling
distribution (a CachyOS build would even require an x86-64-v3 CPU). Ubuntu 24.04 (glibc 2.39)
is the base the script was verified on and the one the workflow uses. From a full checkout
(for a git worktree, mount a bare clone as in the Arch section instead):

```sh
mkdir -p out
podman run --rm -it -v "$PWD":/src:ro,Z -v "$PWD/out":/out:Z docker.io/library/ubuntu:24.04 bash
apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git \
    build-essential pkg-config file libmpv-dev libfontconfig1-dev desktop-file-utils appstream
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . ~/.cargo/env
git config --global --add safe.directory '*'
git clone /src /build && bash /build/rust/packaging/appimage/build-appimage.sh /out
```

The result is about 106 MB. Run it with `chmod +x Feishin-*.AppImage && ./Feishin-*.AppImage`;
on systems without FUSE use `./Feishin-*.AppImage --appimage-extract-and-run`. A headless smoke
test that expects exit status 0 (the app saves its state and quits on `SIGTERM`):

```sh
xvfb-run -a dbus-run-session -- timeout --preserve-status -s TERM 5 ./Feishin-*.AppImage --demo
echo $?
```

Run it through FUSE or on an extracted AppImage (`--appimage-extract`, then
`squashfs-root/AppRun`): in extract-and-run mode the AppImage runtime does not forward
`SIGTERM` to the app.

The **Rust AppImage** GitHub Actions workflow (`.github/workflows/rust-appimage.yml`) runs the
same script on Ubuntu 24.04 when started by hand and uploads the AppImage as a build artifact.
It never publishes a release.
