import { useEffect } from "react";
import * as tools from "../edit/tools";
import { getState, requestFit } from "../state";
import { dispatch, randomFill, stepOnce, togglePlay } from "./actions";

const isTextField = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** Enter play/pause, N step, R random fill, F fit, and the editing keys of `tools.key` (S, D, H, V, Escape, Delete, Backspace). Returns whether the key was handled. */
export function handleShortcut(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey || isTextField(e.target)) return false;
  switch (e.key) {
    case "Enter":
      void togglePlay();
      break;
    case "n":
    case "N":
      stepOnce();
      break;
    case "r":
    case "R":
      void randomFill();
      break;
    case "f":
    case "F":
      requestFit();
      break;
    default: {
      const r = tools.key(getState().edit, e.key);
      if (!r) return false;
      dispatch(r);
    }
  }
  e.preventDefault();
  return true;
}

export function useShortcuts() {
  useEffect(() => {
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, []);
}
