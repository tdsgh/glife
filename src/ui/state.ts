import { useSyncExternalStore } from "react";
import { LAYER_AGE, LAYER_TRAIL } from "./backend";
import type { FrameHeader } from "./frame";
import { initialEdit, type EditState } from "./edit/tools";
import { PALETTE_SIZES, PALETTES, type PaletteId, type PaletteSize } from "./color/palettes";

export interface ViewSettings {
  coloring: "plain" | "age";
  palette: PaletteId;
  paletteSize: PaletteSize;
  trail: boolean;
  /** Generations, 1-100. */
  trailLength: number;
}

const DEFAULT_SETTINGS: ViewSettings = { coloring: "plain", palette: "viridis", paletteSize: 32, trail: false, trailLength: 10 };
const SETTINGS_KEY = "glife.viewSettings";

function loadSettings(): ViewSettings {
  try {
    const raw = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null") ?? {};
    const s = { ...DEFAULT_SETTINGS };
    if (raw.coloring === "plain" || raw.coloring === "age") s.coloring = raw.coloring;
    if (raw.palette in PALETTES) s.palette = raw.palette;
    if (PALETTE_SIZES.includes(raw.paletteSize)) s.paletteSize = raw.paletteSize;
    if (typeof raw.trail === "boolean") s.trail = raw.trail;
    if (Number.isInteger(raw.trailLength) && raw.trailLength >= 1 && raw.trailLength <= 100) s.trailLength = raw.trailLength;
    return s;
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export interface UiState {
  header: FrameHeader | null;
  /** Generations per second; null is as fast as possible. */
  speed: number | null;
  /** Random-fill density as typed, percent. */
  density: string;
  settings: ViewSettings;
  edit: EditState;
  /** One line for an error or notice; null when there is none. */
  notice: string | null;
}

export const SPEEDS: (number | null)[] = [1, 2, 5, 10, 15, 30, 60, 120, 240, null];

let state: UiState = { header: null, speed: 10, density: "25", settings: loadSettings(), edit: initialEdit(), notice: null };
const listeners = new Set<() => void>();
const fitListeners = new Set<() => void>();

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

export const getState = () => state;
export const useUiState = () => useSyncExternalStore(subscribe, getState);

function set(patch: Partial<UiState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

const sameHeader = (a: FrameHeader | null, b: FrameHeader) =>
  a !== null &&
  a.playing === b.playing &&
  a.generation === b.generation &&
  a.liveCount === b.liveCount &&
  a.gensPerSec === b.gensPerSec &&
  a.gridWidth === b.gridWidth &&
  a.gridHeight === b.gridHeight &&
  a.edges === b.edges &&
  a.canUndo === b.canUndo &&
  a.canRedo === b.canRedo &&
  a.dirty === b.dirty;

export function setHeader(header: FrameHeader) {
  if (!sameHeader(state.header, header)) set({ header });
}
export const setSpeed = (speed: number | null) => set({ speed });
export const setEdit = (edit: EditState) => set({ edit });
export const setNotice = (notice: string | null) => set({ notice });
export const setDensity = (density: string) => set({ density });

export function setSettings(patch: Partial<ViewSettings>) {
  const settings = { ...state.settings, ...patch };
  set({ settings });
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    // Storage unavailable: the settings last for this session only.
  }
}

/** The layers `frame` should carry: the trail is drawn only in age coloring. */
export function wantedLayers(): number {
  const { coloring, trail } = state.settings;
  return coloring === "age" ? (trail ? LAYER_AGE | LAYER_TRAIL : LAYER_AGE) : 0;
}

/** The density to send: the typed value rounded and clamped to 1-99, 25 when it is not a number. */
export function densityValue(): number {
  const n = Math.round(Number(state.density));
  return state.density.trim() === "" || Number.isNaN(n) ? 25 : Math.min(99, Math.max(1, n));
}

/** Asks the view to fit the grid to the window. */
export const requestFit = () => fitListeners.forEach((l) => l());
export function onFit(l: () => void) {
  fitListeners.add(l);
  return () => fitListeners.delete(l);
}

export function resetUiState() {
  state = { header: null, speed: 10, density: "25", settings: { ...DEFAULT_SETTINGS }, edit: initialEdit(), notice: null };
  listeners.forEach((l) => l());
}
