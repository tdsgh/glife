import { useEffect, useRef } from "react";
import * as THREE from "three";
import { edit, fetchFrame, NO_VERSION, type Region } from "../backend";
import { getState, onFit, setHeader, wantedLayers } from "../state";
import { paletteColors } from "../color/palettes";
import { decodeFrame, type FrameHeader } from "../frame";
import { fragmentShader, vertexShader } from "./gridShader";
import { startStroke, strokeMode, type Cell } from "../edit/stroke";
import { fit, panBy, resize, screenToCell, visibleRegion, zoomAt, type Viewport } from "./viewport";

const PROBE: Region = { x0: 0, y0: 0, w: 32, h: 1 };

const sameRegion = (a: Region, b: Region) =>
  a.x0 === b.x0 && a.y0 === b.y0 && a.w === b.w && a.h === b.h;

export function GridView() {
  const hostRef = useRef<HTMLDivElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = hostRef.current!;
    const overlay = overlayRef.current!;

    const renderer = new THREE.WebGLRenderer({ antialias: false });
    renderer.setPixelRatio(window.devicePixelRatio);
    host.appendChild(renderer.domElement);
    renderer.domElement.style.display = "block";

    const placeholder = () => {
      const t = new THREE.DataTexture(new Uint32Array(1), 1, 1, THREE.RedIntegerFormat, THREE.UnsignedIntType);
      t.internalFormat = "R32UI";
      t.minFilter = t.magFilter = THREE.NearestFilter;
      t.needsUpdate = true;
      return t;
    };
    let texture = placeholder();

    const byteTexture = (data: Uint8Array, w: number, h: number) => {
      const t = new THREE.DataTexture(data, w, h, THREE.RedIntegerFormat, THREE.UnsignedByteType);
      t.internalFormat = "R8UI";
      t.minFilter = t.magFilter = THREE.NearestFilter;
      t.needsUpdate = true;
      return t;
    };
    let agesTexture = byteTexture(new Uint8Array(1), 1, 1);
    let trailTexture = byteTexture(new Uint8Array(1), 1, 1);
    // Uploads a byte layer, reallocating the texture only when its size changes.
    const uploadBytes = (tex: THREE.DataTexture, data: Uint8Array, w: number, h: number) => {
      if (tex.image.width !== w || tex.image.height !== h) {
        tex.dispose();
        return byteTexture(data, w, h);
      }
      tex.image.data = data;
      tex.needsUpdate = true;
      return tex;
    };
    let paletteKey = "";
    let paletteTexture = new THREE.DataTexture(new Uint8Array(4), 1, 1);

    const material = new THREE.ShaderMaterial({
      glslVersion: THREE.GLSL3,
      vertexShader,
      fragmentShader,
      uniforms: {
        uCells: { value: texture },
        uRegion: { value: new THREE.Vector4(0, 0, 0, 0) },
        uGrid: { value: new THREE.Vector2(1, 1) },
        uCenter: { value: new THREE.Vector2(0, 0) },
        uWin: { value: new THREE.Vector2(1, 1) },
        uPpc: { value: 1 },
        uMode: { value: 0 },
        uAges: { value: agesTexture },
        uTrail: { value: trailTexture },
        uPalette: { value: paletteTexture },
        uPaletteN: { value: 1 },
        uTrailLen: { value: 0 },
        uAliveColor: { value: new THREE.Color(0xe8f1ff) },
        uDeadColor: { value: new THREE.Color(0x14171c) },
        uLineColor: { value: new THREE.Color(0x4a5260) },
        uOutsideColor: { value: new THREE.Color(0x07080a) },
        uBoundaryColor: { value: new THREE.Color(0x8892a6) },
      },
    });
    const scene = new THREE.Scene();
    scene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material));
    const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

    let view: Viewport | null = null;
    let size = { width: host.clientWidth || 1, height: host.clientHeight || 1 };
    let gridKey = "";
    let uploaded: Region | null = null;
    let since = NO_VERSION;
    // Layers the uploaded textures hold, and the ones the last request asked for.
    let uploadedLayers = -1;
    let haveAges = false;
    let haveTrail = false;
    let inFlight = false;
    let disposed = false;
    let spaceHeld = false;
    let showOverlay = false;

    let header: FrameHeader | null = null;
    let lastAlive: Uint32Array | null = null;
    let reqMs = 0;
    let frameBytes = 0;
    let frames = 0;
    let fps = 0;
    let lastFpsAt = performance.now();

    const applySize = () => {
      renderer.setSize(size.width, size.height);
      if (view) view = resize(view, size.width, size.height);
    };
    applySize();

    const request = () => {
      inFlight = true;
      const region = view ? visibleRegion(view) : PROBE;
      const layers = wantedLayers();
      const wanted = view && uploaded && uploadedLayers === layers && sameRegion(region, uploaded) ? since : NO_VERSION;
      const t0 = performance.now();
      fetchFrame(region, wanted, layers)
        .then((buf) => {
          if (disposed) return;
          reqMs = performance.now() - t0;
          frameBytes = buf.byteLength;
          const frame = decodeFrame(buf);
          const h = frame.header;
          header = h;
          setHeader(h);
          const key = `${h.gridWidth}x${h.gridHeight}`;
          if (key !== gridKey) {
            gridKey = key;
            view = fit(size.width, size.height, h.gridWidth, h.gridHeight);
            uploaded = null;
            since = NO_VERSION;
          }
          if (frame.alive && h.w > 0 && h.h > 0) {
            const wWords = h.w / 32;
            if (texture.image.width !== wWords || texture.image.height !== h.h) {
              texture.dispose();
              texture = new THREE.DataTexture(frame.alive, wWords, h.h, THREE.RedIntegerFormat, THREE.UnsignedIntType);
              texture.internalFormat = "R32UI";
              texture.minFilter = texture.magFilter = THREE.NearestFilter;
              material.uniforms.uCells.value = texture;
            } else {
              texture.image.data = frame.alive;
            }
            texture.needsUpdate = true;
            lastAlive = frame.alive;
            haveAges = frame.ages !== null;
            haveTrail = frame.trail !== null;
            if (frame.ages) {
              agesTexture = uploadBytes(agesTexture, frame.ages, h.w, h.h);
              material.uniforms.uAges.value = agesTexture;
            }
            if (frame.trail) {
              trailTexture = uploadBytes(trailTexture, frame.trail, h.w, h.h);
              material.uniforms.uTrail.value = trailTexture;
            }
            uploadedLayers = layers;
            uploaded = { x0: h.x0, y0: h.y0, w: h.w, h: h.h };
            material.uniforms.uRegion.value.set(h.x0, h.y0, h.w, h.h);
            since = h.snapshotVersion;
          }
        })
        .catch(() => {})
        .finally(() => {
          inFlight = false;
        });
    };

    const updateOverlay = (now: number) => {
      frames++;
      if (now - lastFpsAt < 500) return;
      fps = (frames * 1000) / (now - lastFpsAt);
      frames = 0;
      lastFpsAt = now;
      overlay.style.display = showOverlay ? "block" : "none";
      if (showOverlay) {
        overlay.textContent =
          `fps ${fps.toFixed(0)}  req ${reqMs.toFixed(1)} ms  ${frameBytes} B/frame  ` +
          `${(header?.gensPerSec ?? 0).toFixed(0)} gen/s`;
      }
    };

    let raf = 0;
    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (!inFlight) request();
      if (view) {
        const dpr = renderer.getPixelRatio();
        const u = material.uniforms;
        u.uGrid.value.set(view.gridW, view.gridH);
        u.uCenter.value.set(view.centerX, view.centerY);
        u.uWin.value.set(size.width * dpr, size.height * dpr);
        u.uPpc.value = view.pxPerCell * dpr;
        const { settings } = getState();
        const key = `${settings.palette}/${settings.paletteSize}`;
        if (key !== paletteKey) {
          paletteKey = key;
          paletteTexture.dispose();
          paletteTexture = new THREE.DataTexture(paletteColors(settings.palette, settings.paletteSize), settings.paletteSize, 1);
          paletteTexture.minFilter = paletteTexture.magFilter = THREE.NearestFilter;
          paletteTexture.needsUpdate = true;
          u.uPalette.value = paletteTexture;
          u.uPaletteN.value = settings.paletteSize;
        }
        u.uMode.value = settings.coloring === "age" && haveAges ? 1 : 0;
        u.uTrailLen.value = settings.coloring === "age" && settings.trail && haveTrail ? settings.trailLength : 0;
        renderer.render(scene, camera);
      }
      updateOverlay(now);
    };
    raf = requestAnimationFrame(loop);

    const ro = new ResizeObserver(() => {
      size = { width: host.clientWidth || 1, height: host.clientHeight || 1 };
      applySize();
    });
    ro.observe(host);

    const local = (e: { clientX: number; clientY: number }) => {
      const r = host.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (!view) return;
      if (e.ctrlKey) {
        const p = local(e);
        const factor = Math.exp(-e.deltaY * 0.01);
        view = zoomAt(view, p.x, p.y, factor);
      } else {
        view = panBy(view, e.deltaX, e.deltaY);
      }
    };

    const cellAt = (e: { clientX: number; clientY: number }): Cell | null => {
      if (!view) return null;
      const p = local(e);
      const c = screenToCell(view, p.x, p.y);
      const x = Math.floor(c.x);
      const y = Math.floor(c.y);
      return x >= 0 && y >= 0 && x < view.gridW && y < view.gridH ? [x, y] : null;
    };
    const isAlive = ([x, y]: Cell) => {
      if (!lastAlive || !uploaded) return false;
      const cx = x - uploaded.x0;
      const cy = y - uploaded.y0;
      if (cx < 0 || cy < 0 || cx >= uploaded.w || cy >= uploaded.h) return false;
      return ((lastAlive[cy * (uploaded.w / 32) + (cx >> 5)] >>> (cx & 31)) & 1) === 1;
    };
    const sendEdit = (cells: Cell[], alive: boolean) => {
      const inGrid = view ? cells.filter(([x, y]) => x >= 0 && y >= 0 && x < view!.gridW && y < view!.gridH) : [];
      if (inGrid.length) edit(inGrid, alive).catch(() => {});
    };

    let drag: { x: number; y: number } | null = null;
    let stroke: ((points: Cell[]) => void) | null = null;
    const onPointerDown = (e: PointerEvent) => {
      if (e.button === 0 && !spaceHeld) {
        const c = cellAt(e);
        if (!c) return;
        stroke = startStroke(c, strokeMode(isAlive(c)), sendEdit);
        host.setPointerCapture(e.pointerId);
        e.preventDefault();
        return;
      }
      if (e.button === 1 || e.button === 2 || (e.button === 0 && spaceHeld)) {
        drag = { x: e.clientX, y: e.clientY };
        host.setPointerCapture(e.pointerId);
        e.preventDefault();
      }
    };
    const onPointerMove = (e: PointerEvent) => {
      if (stroke && view) {
        // Unclamped points: cells outside the grid are filtered at send time.
        const events = e.getCoalescedEvents?.() ?? [];
        const points = (events.length ? events : [e]).map((ev): Cell => {
          const p = local(ev);
          const c = screenToCell(view!, p.x, p.y);
          return [Math.floor(c.x), Math.floor(c.y)];
        });
        stroke(points);
        return;
      }
      if (!drag || !view) return;
      view = panBy(view, drag.x - e.clientX, drag.y - e.clientY);
      drag = { x: e.clientX, y: e.clientY };
    };
    const onPointerUp = () => {
      drag = null;
      stroke = null;
    };
    const onContextMenu = (e: Event) => e.preventDefault();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        spaceHeld = true;
        e.preventDefault();
      } else if (e.code === "Backquote") {
        showOverlay = !showOverlay;
        lastFpsAt = 0;
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") spaceHeld = false;
    };

    const offFit = onFit(() => {
      if (view) view = fit(size.width, size.height, view.gridW, view.gridH);
    });

    host.addEventListener("wheel", onWheel, { passive: false });
    host.addEventListener("pointerdown", onPointerDown);
    host.addEventListener("pointermove", onPointerMove);
    host.addEventListener("pointerup", onPointerUp);
    host.addEventListener("pointercancel", onPointerUp);
    host.addEventListener("contextmenu", onContextMenu);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      disposed = true;
      cancelAnimationFrame(raf);
      ro.disconnect();
      offFit();
      host.removeEventListener("wheel", onWheel);
      host.removeEventListener("pointerdown", onPointerDown);
      host.removeEventListener("pointermove", onPointerMove);
      host.removeEventListener("pointerup", onPointerUp);
      host.removeEventListener("pointercancel", onPointerUp);
      host.removeEventListener("contextmenu", onContextMenu);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      texture.dispose();
      agesTexture.dispose();
      trailTexture.dispose();
      paletteTexture.dispose();
      material.dispose();
      renderer.dispose();
      renderer.domElement.remove();
    };
  }, []);

  return (
    <div ref={hostRef} style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden" }}>
      <div
        ref={overlayRef}
        style={{
          display: "none",
          position: "absolute",
          top: 8,
          left: 8,
          padding: "2px 6px",
          font: "12px monospace",
          color: "#e8f1ff",
          background: "rgba(0,0,0,0.6)",
          pointerEvents: "none",
        }}
      />
    </div>
  );
}
