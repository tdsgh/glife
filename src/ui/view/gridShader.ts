export const vertexShader = /* glsl */ `
void main() {
  gl_Position = vec4(position.xy, 0.0, 1.0);
}
`;

export const fragmentShader = /* glsl */ `
precision highp float;
precision highp int;
precision highp usampler2D;

uniform usampler2D uCells;   // one texel per 32 cells of a row
uniform vec4 uRegion;        // x0, y0, w, h in cells
uniform vec2 uGrid;          // grid width, height in cells
uniform vec2 uCenter;        // view centre in world units
uniform int uTiling;         // 0 square, 1 hexagonal, 2 and 3 triangular
uniform vec2 uPitch;         // world distance between neighboring columns, rows
uniform vec2 uWin;           // drawing buffer size in pixels
uniform float uPpc;          // device pixels per world unit
uniform int uMode;           // 0 plain, 1 age
uniform usampler2D uAges;    // one texel per cell, region-sized
uniform usampler2D uTrail;   // same shape; counter 0 means no trail
uniform sampler2D uPalette;  // uPaletteN entries
uniform int uPaletteN;
uniform int uTrailLen;       // 0 when the trail is off
uniform usampler2D uPlace;   // the pattern being placed: one texel per 32 cells of a row
uniform int uPlaceOn;
uniform vec2 uPlaceOrigin;   // top-left cell of the pattern
uniform vec2 uPlaceSize;     // width, height in cells
uniform vec2 uPlaceFlip;     // 1 when flipped horizontally, vertically
uniform int uWrap;           // 1 when the grid wraps
uniform vec4 uSel;           // selection x, y, w, h in cells
uniform int uSelOn;
uniform vec3 uPlaceColor;
uniform vec3 uSelColor;
uniform vec3 uAliveColor;
uniform vec3 uDeadColor;
uniform vec3 uLineColor;
uniform vec3 uOutsideColor;
uniform vec3 uBoundaryColor;

out vec4 outColor;

const float ROW = 0.8660254;       // triangle height, hexagon row pitch
const float HEX_HALF = 0.5773503;  // half the height of a hexagon of width 1

vec2 cellCenter(ivec2 c) {
  if (uTiling == 1) return vec2(float(c.x) + 0.5 + float(c.y & 1) * 0.5, float(c.y) * ROW + HEX_HALF);
  if (uTiling >= 2) return vec2(float(c.x + 1) * 0.5, (float(c.y) + 0.5) * ROW);
  return vec2(c) + 0.5;
}

// The cell under a world point; outside the grid it gives cells outside it.
ivec2 cellAt(vec2 w) {
  if (uTiling == 1) {
    // Hexagons are the Voronoi cells of their centers: nearest of three candidate rows.
    int r = int(round((w.y - HEX_HALF) / ROW));
    ivec2 best = ivec2(0, r);
    float bestD = 1e9;
    for (int dy = -1; dy <= 1; dy++) {
      int y = r + dy;
      int x = int(round(w.x - 0.5 - float(y & 1) * 0.5));
      vec2 d = w - cellCenter(ivec2(x, y));
      float dd = dot(d, d);
      if (dd < bestD) {
        bestD = dd;
        best = ivec2(x, y);
      }
    }
    return best;
  }
  if (uTiling >= 2) {
    int y = int(floor(w.y / ROW));
    float fy = w.y / ROW - float(y);
    int i = int(floor(w.x * 2.0));
    for (int dx = -1; dx <= 1; dx++) {
      int x = i + dx;
      float span = (((x + y) & 1) == 0 ? fy : 1.0 - fy) * 0.5;
      if (abs(w.x - float(x + 1) * 0.5) <= span) return ivec2(x, y);
    }
    return ivec2(i, y);
  }
  return ivec2(floor(w));
}

bool inGrid(ivec2 c) {
  return c.x >= 0 && c.y >= 0 && c.x < int(uGrid.x) && c.y < int(uGrid.y);
}

// World distance from w, a point inside cell c, to the nearest edge of c.
float edgeDist(vec2 w, ivec2 c) {
  if (uTiling == 1) {
    vec2 d = w - cellCenter(c);
    float a = abs(d.x);
    float b = abs(dot(d, vec2(0.5, ROW)));
    float e = abs(dot(d, vec2(-0.5, ROW)));
    return 0.5 - max(a, max(b, e));
  }
  if (uTiling >= 2) {
    float top = float(c.y) * ROW;
    vec2 q = w;
    if (((c.x + c.y) & 1) != 0) q.y = 2.0 * top + ROW - w.y;  // a down triangle, mirrored to an up one
    vec2 d = q - vec2(cellCenter(c).x, top);
    return min(top + ROW - q.y, 0.5 * d.y - ROW * abs(d.x));
  }
  vec2 f = fract(w);
  return min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
}

bool selected(ivec2 c) {
  return c.x >= int(uSel.x) && c.y >= int(uSel.y) && c.x < int(uSel.x + uSel.z) && c.y < int(uSel.y + uSel.w);
}

bool alive(ivec2 c) {
  if (c.x < 0 || c.y < 0 || c.x >= int(uGrid.x) || c.y >= int(uGrid.y)) return false;
  ivec2 r = c - ivec2(uRegion.xy);
  if (r.x < 0 || r.y < 0 || r.x >= int(uRegion.z) || r.y >= int(uRegion.w)) return false;
  uint word = texelFetch(uCells, ivec2(r.x >> 5, r.y), 0).r;
  return ((word >> uint(r.x & 31)) & 1u) == 1u;
}

// Age-coloring color of a cell; false when it is neither alive nor trailing.
bool ageColor(ivec2 c, out vec3 col) {
  if (c.x < 0 || c.y < 0 || c.x >= int(uGrid.x) || c.y >= int(uGrid.y)) return false;
  ivec2 r = c - ivec2(uRegion.xy);
  if (r.x < 0 || r.y < 0 || r.x >= int(uRegion.z) || r.y >= int(uRegion.w)) return false;
  uint word = texelFetch(uCells, ivec2(r.x >> 5, r.y), 0).r;
  bool live = ((word >> uint(r.x & 31)) & 1u) == 1u;
  uint d = live || uTrailLen == 0 ? 0u : texelFetch(uTrail, r, 0).r;
  if (!live && (d < 1u || d > uint(uTrailLen))) return false;
  int age = min(int(texelFetch(uAges, r, 0).r), uPaletteN - 1);
  col = texelFetch(uPalette, ivec2(age, 0), 0).rgb;
  if (!live) col = mix(col, uDeadColor, float(d) / float(uTrailLen + 1));
  return true;
}

// 0 outside the pattern, 1 inside its rectangle, 2 on one of its live cells.
int placeAt(ivec2 c) {
  ivec2 size = ivec2(uPlaceSize);
  ivec2 d = c - ivec2(uPlaceOrigin);
  if (uWrap == 1) d = ivec2((d.x % int(uGrid.x) + int(uGrid.x)) % int(uGrid.x), (d.y % int(uGrid.y) + int(uGrid.y)) % int(uGrid.y));
  if (d.x < 0 || d.y < 0 || d.x >= size.x || d.y >= size.y) return 0;
  if (uPlaceFlip.x > 0.5) d.x = size.x - 1 - d.x;
  if (uPlaceFlip.y > 0.5) d.y = size.y - 1 - d.y;
  uint word = texelFetch(uPlace, ivec2(d.x >> 5, d.y), 0).r;
  return ((word >> uint(d.x & 31)) & 1u) == 1u ? 2 : 1;
}

void main() {
  vec2 p = vec2(gl_FragCoord.x, uWin.y - gl_FragCoord.y);
  vec2 c = uCenter + (p - uWin * 0.5) / uPpc;
  ivec2 cell = cellAt(c);
  bool inside = inGrid(cell);

  vec3 color = inside ? uDeadColor : uOutsideColor;

  if (uPpc * min(uPitch.x, uPitch.y) >= 1.0) {
    vec3 cc;
    if (uMode == 1) {
      if (ageColor(cell, cc)) color = cc;
    } else if (alive(cell)) color = uAliveColor;
    if (inside) {
      float d = edgeDist(c, cell) * uPpc;
      float fade = clamp((uPpc - 6.0) / 6.0, 0.0, 1.0);
      float line = (1.0 - smoothstep(0.5, 1.0, d)) * fade;
      color = mix(color, uLineColor, line * 0.5);
    }
  } else {
    // Cells under one pixel: sample the stored columns and rows the pixel covers.
    int kx = int(min(ceil(1.0 / (uPpc * uPitch.x)), 16.0));
    int ky = int(min(ceil(1.0 / (uPpc * uPitch.y)), 16.0));
    int live = 0;
    vec3 sum = vec3(0.0);
    for (int j = 0; j < 16; j++) {
      if (j >= ky) break;
      for (int i = 0; i < 16; i++) {
        if (i >= kx) break;
        if (uMode == 1) {
          vec3 cc;
          if (ageColor(cell + ivec2(i, j), cc)) {
            live++;
            sum += cc;
          }
        } else if (alive(cell + ivec2(i, j))) live++;
      }
    }
    if (live > 0) {
      float share = float(live) / float(kx * ky);
      vec3 target = uMode == 1 ? sum / float(live) : uAliveColor;
      color = mix(uDeadColor, target, max(share, 0.25));
    }
  }

  if (uPlaceOn == 1 && inside) {
    int pl = placeAt(cell);
    if (pl == 1) color = mix(color, uPlaceColor, 0.12);
    if (pl == 2) color = mix(color, uPlaceColor, 0.6);
  }

  float onePx = 1.0 / uPpc;
  vec2 probe[4] = vec2[4](vec2(1.5, 0.0), vec2(-1.5, 0.0), vec2(0.0, 1.5), vec2(0.0, -1.5));

  // Selection outline, one pixel wide: where a selected cell meets an unselected one.
  if (uSelOn == 1) {
    if (uTiling == 0) {
      float sx = min(abs(c.x - uSel.x), abs(c.x - (uSel.x + uSel.z)));
      float sy = min(abs(c.y - uSel.y), abs(c.y - (uSel.y + uSel.w)));
      bool onX = sx < onePx && c.y > uSel.y - onePx && c.y < uSel.y + uSel.w + onePx;
      bool onY = sy < onePx && c.x > uSel.x - onePx && c.x < uSel.x + uSel.z + onePx;
      if (onX || onY) color = uSelColor;
    } else if (selected(cell) && edgeDist(c, cell) < onePx) {
      for (int k = 0; k < 4; k++) {
        if (!selected(cellAt(c + probe[k] * onePx))) color = uSelColor;
      }
    }
  }

  // Grid boundary, one pixel wide.
  if (uTiling == 0) {
    float bx = min(abs(c.x), abs(c.x - uGrid.x));
    float by = min(abs(c.y), abs(c.y - uGrid.y));
    bool nearX = bx < onePx && c.y > -onePx && c.y < uGrid.y + onePx;
    bool nearY = by < onePx && c.x > -onePx && c.x < uGrid.x + onePx;
    if (nearX || nearY) color = uBoundaryColor;
  } else if (inside && edgeDist(c, cell) < onePx) {
    for (int k = 0; k < 4; k++) {
      if (!inGrid(cellAt(c + probe[k] * onePx))) color = uBoundaryColor;
    }
  }

  outColor = vec4(color, 1.0);
}
`;
