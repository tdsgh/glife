import { useEffect } from "react";
import { clear } from "../backend";
import { requestFit } from "../state";
import { randomFill, stepOnce, togglePlay } from "./actions";

const isTextField = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** Enter play/pause, N step, R random fill, Backspace/Delete clear, F fit. Returns whether the key was handled. */
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
    case "Backspace":
    case "Delete":
      void clear();
      break;
    case "f":
    case "F":
      requestFit();
      break;
    default:
      return false;
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
