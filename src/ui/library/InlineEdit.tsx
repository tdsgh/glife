import { useRef, useState } from "react";
import type { LibraryResult } from "../backend";

interface Props {
  kind: "name" | "comment";
  initial: string;
  /** Resolves to `{ nameTaken: true }` when the name is in use and `replace` was false. */
  submit: (value: string, replace: boolean) => Promise<LibraryResult>;
  /** Called once, after a save or a cancel. */
  onClose: () => void;
}

/** A name field or comment box that edits where it stands: Enter or leaving saves, Escape cancels. */
export function InlineEdit({ kind, initial, submit, onClose }: Props) {
  const [value, setValue] = useState(initial);
  const [taken, setTaken] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const field = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const label = kind === "name" ? "Name" : "Comment";

  const send = async (replace: boolean) => {
    const v = kind === "name" ? value.trim() : value;
    if (busy.current) return;
    if (v === initial || (kind === "name" && v === "")) return onClose();
    busy.current = true;
    try {
      const reply = await submit(v, replace);
      if (reply && reply.nameTaken) setTaken(true);
      else return onClose();
    } catch (e) {
      setError(String(e));
    }
    busy.current = false;
  };

  const props = {
    ref: field,
    "aria-label": label,
    value,
    autoFocus: true,
    onFocus: (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement>) => e.target.select(),
    onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => { setValue(e.target.value); setTaken(false); },
    onBlur: () => { if (!taken) void send(false); },
    onKeyDown: (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        busy.current = true;
        onClose();
      } else if (e.key === "Enter" && !(kind === "comment" && e.shiftKey)) {
        e.preventDefault();
        void send(false);
      }
    },
  };

  return (
    <div onClick={(e) => e.stopPropagation()} style={{ flex: 1 }}>
      {kind === "name"
        ? <input type="text" {...(props as object)} style={{ width: "100%", boxSizing: "border-box" }} />
        : <textarea rows={3} {...(props as object)} style={{ width: "100%", boxSizing: "border-box" }} />}
      {taken && (
        <div role="alert" style={{ margin: "4px 0", color: "#ffe9a8" }}>
          A pattern named “{value.trim()}” already exists.{" "}
          <button onClick={() => void send(true)}>Replace</button>{" "}
          <button onClick={() => { setTaken(false); field.current?.focus(); }}>Choose another name</button>
        </div>
      )}
      {error !== null && <div role="alert" style={{ margin: "4px 0", color: "#ff8a80" }}>{error}</div>}
    </div>
  );
}
