import { AbsoluteFill, Sequence, staticFile } from "remotion";
import { Backdrop } from "./Backdrop";
import { Logo } from "./Logo";
import { Pan, Shot } from "./Shot";
import { OVERLAP, SCENE } from "./theme";
import { ThemeSwap } from "./ThemeSwap";

const INTRO = 66;
const SWAP = 84;
const OUTRO = 78;

const shots: { src: string; caption: string; pan: Pan }[] = [
  {
    src: "dark-home.png",
    caption: "Your Navidrome, OpenSubsonic or Jellyfin library",
    pan: { fromScale: 0.94, toScale: 1, fromX: 0, toX: 0, fromY: -10, toY: -26 },
  },
  {
    src: "dark-album.png",
    caption: "Native, fast and light on memory",
    pan: { fromScale: 1.06, toScale: 1, fromX: 30, toX: 0, fromY: -26, toY: -26 },
  },
  {
    src: "dark-now-playing.png",
    caption: "Now Playing with synced lyrics",
    pan: { fromScale: 0.94, toScale: 1, fromX: -30, toX: 0, fromY: -26, toY: -26 },
  },
  {
    src: "dark-discover.png",
    caption: "YouTube Music discovery and daily mixes",
    pan: { fromScale: 1.06, toScale: 1, fromX: 0, toX: 0, fromY: -6, toY: -26 },
  },
];

const shotStart = (index: number) => INTRO - OVERLAP + index * (SCENE - OVERLAP);
const swapStart = shotStart(shots.length);
const outroStart = swapStart + SWAP - OVERLAP;

export const PROMO_DURATION = outroStart + OUTRO;

export const Promo = () => (
  <AbsoluteFill>
    <style>{`@font-face { font-family: Inter; src: url(${staticFile("InterVariable.ttf")}); font-weight: 100 900; }`}</style>
    <Backdrop />
    <Sequence durationInFrames={INTRO}>
      <Logo tagline="A fast, native music player for your self-hosted library" />
    </Sequence>
    {shots.map((shot, index) => (
      <Sequence key={shot.src} from={shotStart(index)} durationInFrames={SCENE}>
        <Shot src={shot.src} caption={shot.caption} pan={shot.pan} duration={SCENE} />
      </Sequence>
    ))}
    <Sequence from={swapStart} durationInFrames={SWAP}>
      <ThemeSwap duration={SWAP} />
    </Sequence>
    <Sequence from={outroStart} durationInFrames={OUTRO}>
      <Logo
        tagline="Personal mixes, player-to-player control and a fraction of the RAM"
        footnote="Free and open source - GPL-3.0"
      />
    </Sequence>
  </AbsoluteFill>
);
