import type { CSSProperties } from "react";

/** Style of a button with an on/off state: same shape and size either way, only the colors change. */
export const toggleStyle = (on: boolean): CSSProperties => ({
  font: "inherit",
  padding: "1px 6px",
  border: "1px solid",
  borderRadius: 4,
  ...(on
    ? { background: "#3a6df0", color: "#fff", borderColor: "#8fb0ff" }
    : { background: "#efefef", color: "#000", borderColor: "#767676" }),
});
