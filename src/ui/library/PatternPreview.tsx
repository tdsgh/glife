import { useEffect, useState } from "react";
import { patternPreview, type Source } from "../backend";
import { decodePattern, type Pattern } from "../frame";
import { setNotice } from "../state";

/** One SVG path for the live cells: a horizontal run per `M x y h n v1 h-n z`. */
export function previewPath({ width, height, bits }: Pattern): string {
  const stride = Math.ceil(width / 32);
  const alive = (x: number, y: number) => ((bits[y * stride + (x >> 5)] >>> (x & 31)) & 1) === 1;
  let d = "";
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (!alive(x, y)) continue;
      let n = 1;
      while (x + n < width && alive(x + n, y)) n++;
      d += `M${x} ${y}h${n}v1h-${n}z`;
      x += n;
    }
  }
  return d;
}

const SIZE = 48;

export function PatternPreview({ source }: { source: Source }) {
  const [pattern, setPattern] = useState<Pattern | null>(null);
  const key = JSON.stringify(source);

  useEffect(() => {
    let live = true;
    patternPreview(JSON.parse(key) as Source)
      .then((buf) => live && setPattern(decodePattern(buf)))
      .catch((e) => live && setNotice(String(e)));
    return () => {
      live = false;
    };
  }, [key]);

  return (
    <svg role="img" aria-label="Preview" width={SIZE} height={SIZE} viewBox={`0 0 ${pattern?.width ?? 1} ${pattern?.height ?? 1}`}
      preserveAspectRatio="xMidYMid meet" style={{ background: "#11141a", flex: "none" }}>
      {pattern && <path d={previewPath(pattern)} fill="#7ee0a0" />}
    </svg>
  );
}
