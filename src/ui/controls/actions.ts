import * as backend from "../backend";
import * as tools from "../edit/tools";
import { decodePattern } from "../frame";
import { densityValue, getState, setEdit, setNotice } from "../state";

export const togglePlay = () => (getState().header?.playing ? backend.pause() : backend.play());
export const stepOnce = () => {
  if (!getState().header?.playing) void backend.step();
};
export const randomFill = () => backend.randomFill(densityValue());

const grid = (): tools.Grid => {
  const h = getState().header;
  return { w: h?.gridWidth ?? 0, h: h?.gridHeight ?? 0 };
};
const fail = (e: unknown) => setNotice(String(e));

/** Stores the new editing state and sends the backend calls it asks for. A `stroke` effect is the caller's to start. */
export function dispatch({ state, effects }: tools.Result) {
  setEdit(state);
  for (const e of effects) {
    switch (e.kind) {
      case "place":
        backend.place(e.x, e.y, e.flipH, e.flipV, e.mode).catch(fail);
        break;
      case "deleteSelection":
        backend.deleteSelection(e.rect).catch(fail);
        break;
      case "flipSelection":
        backend.flipSelection(e.rect, e.axis).catch(fail);
        break;
      case "clear":
        backend.clear().catch(fail);
        break;
    }
  }
}

const edit = () => getState().edit;

export const setTool = (tool: tools.Tool) => dispatch(tools.setTool(edit(), tool));
export const setPasteMode = (mode: backend.PasteMode) => dispatch(tools.setMode(edit(), mode));
export const resetEdit = () => dispatch(tools.reset(edit()));
export const flipH = () => dispatch(tools.flip(edit(), "h"));
export const flipV = () => dispatch(tools.flip(edit(), "v"));
export const selectAll = () => dispatch(tools.selectAll(edit(), grid()));
/** Delete: ends placing, else kills the selection, else clears the universe. */
export const deleteAction = () => dispatch(tools.deleteKey(edit()));

export const undo = () => {
  if (!getState().header?.playing) backend.undo().catch(fail);
};
export const redo = () => {
  if (!getState().header?.playing) backend.redo().catch(fail);
};

/** Copy and cut act on the selection; a pattern being placed has nothing to copy. */
export const copy = () => {
  const { placing, selection } = edit();
  if (!placing && selection) backend.copySelection(selection).catch(fail);
};
export const cut = () => {
  const { placing, selection } = edit();
  if (!placing && selection) backend.cutSelection(selection).catch(fail);
};

/** Fetches the backend's pending pattern and starts placing it. */
export async function beginPlacing() {
  dispatch(tools.startPlacing(edit(), decodePattern(await backend.pendingCells())));
}

/** Pastes the clipboard: shows the notices, or the error without starting placing. */
export async function paste() {
  try {
    const info = await backend.paste();
    await beginPlacing();
    if (info.notices?.length) setNotice(info.notices.join(" "));
  } catch (e) {
    fail(e);
  }
}
