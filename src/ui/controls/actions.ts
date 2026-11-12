import * as backend from "../backend";
import { densityValue, getState } from "../state";

export const togglePlay = () => (getState().header?.playing ? backend.pause() : backend.play());
export const stepOnce = () => {
  if (!getState().header?.playing) void backend.step();
};
export const randomFill = () => backend.randomFill(densityValue());
