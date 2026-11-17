import { useEffect } from "react";
import * as tools from "../edit/tools";
import { getState, requestFit } from "../state";
import { dispatch, randomFill, redo, stepOnce, togglePlay } from "./actions";

export const isTextField = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** Enter play/pause, N step, R random fill, F fit, and the editing keys of `tools.key` (S, D, H, V, Escape, Delete, Backspace). Returns whether the key was handled. */
export function handleShortcut(e: KeyboardEvent): boolean {
  // The web view takes Cmd+Shift+Z before the Edit menu sees it on macOS; when the menu does handle it, this event never arrives.
  if (e.metaKey && e.shiftKey && e.code === "KeyZ" && !isTextField(e.target)) {
    redo();
    e.preventDefault();
    return true;
  }
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
