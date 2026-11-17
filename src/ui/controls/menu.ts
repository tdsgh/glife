import { useEffect } from "react";
import * as backend from "../backend";
import { setNotice, useUiState } from "../state";
import * as actions from "./actions";

/** The `menu` event's item ids, as menu.rs names them. */
export const MENU_ACTIONS: Record<string, () => void | Promise<void>> = {
  new: actions.newUniverse,
  open: actions.openFile,
  save: actions.save,
  save_as: actions.saveAs,
  import_rle: actions.importRle,
  export_rle: actions.exportRle,
  undo: actions.undo,
  redo: actions.redo,
  cut: actions.cut,
  copy: actions.copy,
  paste: actions.paste,
  delete: actions.deleteAction,
  select_all: actions.selectAll,
  flip_h: actions.flipH,
  flip_v: actions.flipV,
};

export const handleMenu = (id: string) => void MENU_ACTIONS[id]?.();

const isTextField = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA"].includes(t.tagName));

/** Runs a backend call, reporting a failure as a notice. A synchronous throw (no Tauri runtime) is reported the same way. */
const report = async (call: () => Promise<unknown>) => {
  try {
    await call();
  } catch (e) {
    setNotice(String(e));
  }
};

/** Routes `menu` events to actions, reports text-field focus, and reports the menu flags. */
export function useMenu() {
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    try {
      backend
        .onMenu(handleMenu)
        .then((un) => (gone ? un() : (off = un)))
        .catch(() => {});
    } catch {
      // No Tauri runtime (a plain browser): menu events never arrive.
    }
    const focus = (target: "text" | "universe") => (e: FocusEvent) => {
      if (isTextField(e.target)) void report(() => backend.setEditTarget(target));
    };
    const onIn = focus("text");
    const onOut = focus("universe");
    document.addEventListener("focusin", onIn);
    document.addEventListener("focusout", onOut);
    return () => {
      gone = true;
      off?.();
      document.removeEventListener("focusin", onIn);
      document.removeEventListener("focusout", onOut);
    };
  }, []);

  const { header, edit } = useUiState();
  const canUndo = header?.canUndo ?? false;
  const canRedo = header?.canRedo ?? false;
  const playing = header?.playing ?? false;
  const hasSelection = edit.selection !== null;
  useEffect(() => {
    void report(() => backend.setMenuState({ canUndo, canRedo, hasSelection, playing }));
  }, [canUndo, canRedo, hasSelection, playing]);
}

/** `<file name> — glife`, prefixed with a bullet while there are unsaved changes. */
export const windowTitle = (fileName: string | null, dirty: boolean) =>
  `${dirty ? "• " : ""}${fileName ?? "Untitled"} — glife`;

export function useWindowTitle() {
  const { header, fileName } = useUiState();
  const dirty = header?.dirty ?? false;
  useEffect(() => {
    void report(() => backend.setTitle(windowTitle(fileName, dirty)));
  }, [fileName, dirty]);
}
