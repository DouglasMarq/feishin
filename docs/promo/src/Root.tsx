import { Composition, Still } from "remotion";
import { Icon } from "./Icon";
import { Promo, PROMO_DURATION } from "./Promo";
import { FPS, HEIGHT, WIDTH } from "./theme";

export const Root = () => (
  <>
    <Composition id="Promo" component={Promo} durationInFrames={PROMO_DURATION} fps={FPS} width={WIDTH} height={HEIGHT} />
    <Still id="Icon" component={Icon} width={1024} height={1024} />
  </>
);
