import { useEffect, useRef } from "react";
import * as THREE from "three";
import { fetchFrame, NO_VERSION, type Region } from "../backend";
import { decodeFrame, type FrameHeader } from "../frame";
import { fragmentShader, vertexShader } from "./gridShader";
import { fit, panBy, resize, visibleRegion, zoomAt, type Viewport } from "./viewport";

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
    let inFlight = false;
    let disposed = false;
    let spaceHeld = false;
    let showOverlay = false;

    let header: FrameHeader | null = null;
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
      const wanted = view && uploaded && sameRegion(region, uploaded) ? since : NO_VERSION;
      const t0 = performance.now();
      fetchFrame(region, wanted)
        .then((buf) => {
          if (disposed) return;
          reqMs = performance.now() - t0;
          frameBytes = buf.byteLength;
          const frame = decodeFrame(buf);
          const h = frame.header;
          header = h;
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
      if (e.ctrlKey || e.deltaMode === 1) {
        const p = local(e);
        const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.1));
        view = zoomAt(view, p.x, p.y, factor);
      } else {
        view = panBy(view, e.deltaX, e.deltaY);
      }
    };

    let drag: { x: number; y: number } | null = null;
    const onPointerDown = (e: PointerEvent) => {
      if (e.button === 1 || e.button === 2 || (e.button === 0 && spaceHeld)) {
        drag = { x: e.clientX, y: e.clientY };
        host.setPointerCapture(e.pointerId);
        e.preventDefault();
      }
    };
    const onPointerMove = (e: PointerEvent) => {
      if (!drag || !view) return;
      view = panBy(view, drag.x - e.clientX, drag.y - e.clientY);
      drag = { x: e.clientX, y: e.clientY };
    };
    const onPointerUp = () => {
      drag = null;
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
      host.removeEventListener("wheel", onWheel);
      host.removeEventListener("pointerdown", onPointerDown);
      host.removeEventListener("pointermove", onPointerMove);
      host.removeEventListener("pointerup", onPointerUp);
      host.removeEventListener("pointercancel", onPointerUp);
      host.removeEventListener("contextmenu", onContextMenu);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      texture.dispose();
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
