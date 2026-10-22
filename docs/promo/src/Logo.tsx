import { Img, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { font } from "./theme";

export const Logo = ({ tagline, footnote }: { tagline: string; footnote?: string }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const pop = spring({ frame, fps, config: { damping: 14, mass: 0.7 } });
  const textIn = interpolate(frame, [10, 26], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        fontFamily: font,
        color: "white",
      }}
    >
      <Img
        src={staticFile("cadence.svg")}
        style={{ width: 200, height: 200, transform: `scale(${pop}) rotate(${(1 - pop) * -20}deg)` }}
      />
      <div style={{ fontSize: 64, fontWeight: 800, letterSpacing: -1.5, marginTop: 4, opacity: textIn, transform: `translateY(${(1 - textIn) * 16}px)` }}>
        Cadence
      </div>
      <div style={{ fontSize: 22, fontWeight: 500, opacity: textIn * 0.85, marginTop: 8, maxWidth: 720, textAlign: "center", lineHeight: 1.35 }}>
        {tagline}
      </div>
      {footnote ? (
        <div style={{ fontSize: 16, opacity: textIn * 0.6, marginTop: 18 }}>{footnote}</div>
      ) : null}
    </div>
  );
};
