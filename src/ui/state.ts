import { useSyncExternalStore } from "react";
import type { FrameHeader } from "./frame";

export interface UiState {
  header: FrameHeader | null;
  /** Generations per second; null is as fast as possible. */
  speed: number | null;
  /** Random-fill density as typed, percent. */
  density: string;
}

export const SPEEDS: (number | null)[] = [1, 2, 5, 10, 15, 30, 60, 120, 240, null];

let state: UiState = { header: null, speed: 10, density: "25" };
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
  a.edges === b.edges;

export function setHeader(header: FrameHeader) {
  if (!sameHeader(state.header, header)) set({ header });
}
export const setSpeed = (speed: number | null) => set({ speed });
export const setDensity = (density: string) => set({ density });

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
  state = { header: null, speed: 10, density: "25" };
  listeners.forEach((l) => l());
}
