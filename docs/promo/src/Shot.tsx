import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame } from "remotion";
import { font } from "./theme";

export type Pan = { fromScale: number; toScale: number; fromX: number; toX: number; fromY: number; toY: number };

export const Caption = ({ text, progress }: { text: string; progress: number }) => (
  <div
    style={{
      position: "absolute",
      left: 0,
      right: 0,
      bottom: 14,
      display: "flex",
      justifyContent: "center",
      opacity: progress,
      transform: `translateY(${(1 - progress) * 12}px)`,
    }}
  >
    <div
      style={{
        fontFamily: font,
        fontSize: 20,
        fontWeight: 650,
        color: "white",
        padding: "8px 20px",
        borderRadius: 999,
        background: "rgba(14,12,28,0.72)",
        border: "1px solid rgba(255,255,255,0.14)",
      }}
    >
      {text}
    </div>
  </div>
);

export const Window = ({ src, pan, duration }: { src: string; pan: Pan; duration: number }) => {
  const frame = useCurrentFrame();
  const t = interpolate(frame, [0, Math.min(18, duration)], [0, 1], {
    extrapolateRight: "clamp",
    easing: Easing.out(Easing.cubic),
  });
  const scale = interpolate(t, [0, 1], [pan.fromScale, pan.toScale]);
  const x = interpolate(t, [0, 1], [pan.fromX, pan.toX]);
  const y = interpolate(t, [0, 1], [pan.fromY, pan.toY]);
  return (
    <AbsoluteFill style={{ alignItems: "center", justifyContent: "center" }}>
      <Img
        src={staticFile(src)}
        style={{
          width: 740,
          borderRadius: 14,
          boxShadow: "0 30px 80px rgba(0,0,0,0.55)",
          transform: `translate(${x}px, ${y}px) scale(${scale})`,
        }}
      />
    </AbsoluteFill>
  );
};

export const Shot = ({ src, caption, pan, duration }: { src: string; caption: string; pan: Pan; duration: number }) => {
  const frame = useCurrentFrame();
  const fade = interpolate(frame, [0, 10, duration - 10, duration], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const captionIn = interpolate(frame, [8, 20], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill style={{ opacity: fade }}>
      <Window src={src} pan={pan} duration={duration} />
      <Caption text={caption} progress={captionIn} />
    </AbsoluteFill>
  );
};
