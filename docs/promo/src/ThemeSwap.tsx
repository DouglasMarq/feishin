import { AbsoluteFill, interpolate, useCurrentFrame } from "remotion";
import { Caption, Window } from "./Shot";

const still = { fromScale: 1, toScale: 1, fromX: 0, toX: 0, fromY: -26, toY: -26 };

export const ThemeSwap = ({ duration }: { duration: number }) => {
  const frame = useCurrentFrame();
  const fade = interpolate(frame, [0, 10, duration - 10, duration], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const wipe = interpolate(frame, [20, duration - 24], [100, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const captionIn = interpolate(frame, [8, 20], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill style={{ opacity: fade }}>
      <Window src="light-now-playing.png" pan={still} duration={duration} />
      <AbsoluteFill style={{ clipPath: `inset(0 0 0 ${100 - wipe}%)` }}>
        <Window src="dark-now-playing.png" pan={still} duration={duration} />
      </AbsoluteFill>
      <Caption text="Light and dark, tinted by every cover" progress={captionIn} />
    </AbsoluteFill>
  );
};
