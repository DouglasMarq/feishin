# Cadence promo

The animation at the top of the README and the app icon are rendered here with
[Remotion](https://www.remotion.dev). The project is separate from the Rust workspace.

```sh
cd docs/promo
npm install
npm run studio     # preview and edit in the browser
npm run icon       # out/icon-1024.png from assets/icons/cadence.svg
npm run gif        # out/cadence.gif straight from Remotion (large, about 38 MB)
```

`npm run assets` (run by every script) copies `assets/icons/cadence.svg` and the Inter font from
`rust/app/ui/fonts` into `public/`.

## README GIF

Remotion's own GIF encoder makes a file too large for a README, so the published GIF is encoded
from an MP4 with an optimized palette (960 px, 15 fps, under 5 MB):

```sh
npx remotion render Promo out/cadence.mp4 --codec=h264 --crf=16
ffmpeg -i out/cadence.mp4 -vf "fps=15,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" out/cadence.gif
cp out/cadence.gif ../../assets/readme/cadence.gif
```

The screenshots in `public/` come from `cargo run --release -p feishin -- --demo` (window at
1280x820, captured at 1600 px wide). Retake them when the interface changes.

## App icon

`npm run icon` renders `assets/icons/cadence.svg` at 1024 px. The PNG sizes in `assets/icons`
are made from it:

```sh
for size in 16 24 32 48 64 72 96 128 256 512; do
    sips -z $size $size out/icon-1024.png --out ../../assets/icons/${size}x${size}.png
done
cp ../../assets/icons/512x512.png ../../assets/icons/icon.png
```

If rendering stops at "Waiting for root component to load", Remotion's bundled headless Chrome
is failing on this machine: pass a local Chrome, for example
`npm run icon -- --browser-executable="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"`.

## License

Remotion is free for individuals and for companies of up to 3 people. Larger companies need a
company license: see <https://www.remotion.dev/license>.
