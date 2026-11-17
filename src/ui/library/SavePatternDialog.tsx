import { useState } from "react";
import type { LibraryResult } from "../backend";

interface Props {
  title: string;
  /** Which fields the dialog edits: a save asks for both, a rename for the name, a comment edit for the comment. */
  fields: { name: boolean; comment: boolean };
  initialName?: string;
  initialComment?: string;
  /** Resolves to `{ nameTaken: true }` when the name is in use and `replace` was false. */
  submit: (name: string, comment: string, replace: boolean) => Promise<LibraryResult>;
  onClose: () => void;
}

/** Name and multi-line comment; on a taken name it offers Replace or another name. */
export function SavePatternDialog({ title, fields, initialName = "", initialComment = "", submit, onClose }: Props) {
  const [name, setName] = useState(initialName);
  const [comment, setComment] = useState(initialComment);
  const [taken, setTaken] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const valid = !fields.name || name.trim() !== "";

  const send = async (replace: boolean) => {
    if (!valid) return;
    try {
      const reply = await submit(name.trim(), comment, replace);
      if (reply && reply.nameTaken) setTaken(true);
      else onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div role="dialog" aria-label={title}
      style={{ position: "absolute", top: 48, left: 8, zIndex: 10, padding: 12, background: "#20242c", color: "#e8f1ff", border: "1px solid #4a5260" }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}>
      {fields.name && (
        <label style={{ display: "block", margin: "6px 0" }}>
          Name{" "}
          <input type="text" aria-label="Name" value={name} autoFocus
            onChange={(e) => { setName(e.target.value); setTaken(false); }}
            onKeyDown={(e) => { if (e.key === "Enter") void send(false); }} />
        </label>
      )}
      {fields.comment && (
        <label style={{ display: "block", margin: "6px 0" }}>
          Comment
          <textarea aria-label="Comment" rows={4} value={comment} onChange={(e) => setComment(e.target.value)}
            style={{ display: "block", width: 260 }} />
        </label>
      )}
      {taken && (
        <div role="alert" style={{ margin: "6px 0", color: "#ffe9a8" }}>
          A pattern named “{name.trim()}” already exists.{" "}
          <button onClick={() => void send(true)}>Replace</button>{" "}
          <button onClick={() => setTaken(false)}>Choose another name</button>
        </div>
      )}
      {error !== null && <div role="alert" style={{ margin: "6px 0", color: "#ff8a80" }}>{error}</div>}
      {!taken && <button onClick={() => void send(false)} disabled={!valid}>Save</button>}{" "}
      <button onClick={onClose}>Cancel</button>
    </div>
  );
}
