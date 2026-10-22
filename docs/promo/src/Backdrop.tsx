import { AbsoluteFill } from "remotion";

export const Backdrop = () => (
  <AbsoluteFill
    style={{
      backgroundColor: "#120f22",
      backgroundImage: [
        "radial-gradient(circle at 18% 12%, rgba(99,102,241,0.55), transparent 55%)",
        "radial-gradient(circle at 92% 40%, rgba(217,70,239,0.42), transparent 50%)",
        "radial-gradient(circle at 40% 105%, rgba(245,158,11,0.40), transparent 55%)",
      ].join(","),
    }}
  />
);
