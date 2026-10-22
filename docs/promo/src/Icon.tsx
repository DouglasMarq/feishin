import { AbsoluteFill, Img, staticFile } from "remotion";

export const Icon = () => (
  <AbsoluteFill>
    <Img src={staticFile("cadence.svg")} style={{ width: "100%", height: "100%" }} />
  </AbsoluteFill>
);
