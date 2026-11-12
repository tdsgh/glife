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
uniform vec2 uCenter;        // view centre in cells
uniform vec2 uWin;           // drawing buffer size in pixels
uniform float uPpc;          // device pixels per cell
uniform vec3 uAliveColor;
uniform vec3 uDeadColor;
uniform vec3 uLineColor;
uniform vec3 uOutsideColor;
uniform vec3 uBoundaryColor;

out vec4 outColor;

bool alive(ivec2 c) {
  if (c.x < 0 || c.y < 0 || c.x >= int(uGrid.x) || c.y >= int(uGrid.y)) return false;
  ivec2 r = c - ivec2(uRegion.xy);
  if (r.x < 0 || r.y < 0 || r.x >= int(uRegion.z) || r.y >= int(uRegion.w)) return false;
  uint word = texelFetch(uCells, ivec2(r.x >> 5, r.y), 0).r;
  return ((word >> uint(r.x & 31)) & 1u) == 1u;
}

void main() {
  vec2 p = vec2(gl_FragCoord.x, uWin.y - gl_FragCoord.y);
  vec2 c = uCenter + (p - uWin * 0.5) / uPpc;
  bool inside = c.x >= 0.0 && c.y >= 0.0 && c.x < uGrid.x && c.y < uGrid.y;

  vec3 color = inside ? uDeadColor : uOutsideColor;

  if (uPpc >= 1.0) {
    if (alive(ivec2(floor(c)))) color = uAliveColor;
    if (inside) {
      vec2 f = fract(c);
      float d = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)) * uPpc;
      float fade = clamp((uPpc - 6.0) / 6.0, 0.0, 1.0);
      float line = (1.0 - smoothstep(0.5, 1.0, d)) * fade;
      color = mix(color, uLineColor, line * 0.5);
    }
  } else {
    int k = int(min(ceil(1.0 / uPpc), 16.0));
    ivec2 base = ivec2(floor(c));
    int live = 0;
    for (int j = 0; j < 16; j++) {
      if (j >= k) break;
      for (int i = 0; i < 16; i++) {
        if (i >= k) break;
        if (alive(base + ivec2(i, j))) live++;
      }
    }
    if (live > 0) {
      float share = float(live) / float(k * k);
      color = mix(uDeadColor, uAliveColor, max(share, 0.25));
    }
  }

  // Grid boundary, one pixel wide.
  float onePx = 1.0 / uPpc;
  float bx = min(abs(c.x), abs(c.x - uGrid.x));
  float by = min(abs(c.y), abs(c.y - uGrid.y));
  bool nearX = bx < onePx && c.y > -onePx && c.y < uGrid.y + onePx;
  bool nearY = by < onePx && c.x > -onePx && c.x < uGrid.x + onePx;
  if (nearX || nearY) color = uBoundaryColor;

  outColor = vec4(color, 1.0);
}
`;
