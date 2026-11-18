import { useEffect, useState } from "react";
import * as backend from "../backend";
import type { BuiltinEntry, LibraryList, PersonalEntry, Source } from "../backend";
import { placeSource } from "../controls/actions";
import { bumpLibrary, setNotice, useUiState } from "../state";
import { PatternPreview } from "./PatternPreview";
import { InlineEdit } from "./InlineEdit";

const CATEGORY_NAMES: Record<BuiltinEntry["category"], string> = {
  "still-life": "Still lifes",
  oscillator: "Oscillators",
  spaceship: "Spaceships",
  gun: "Guns",
  methuselah: "Methuselahs",
};
const EMPTY: LibraryList = { builtin: [], personal: [], problems: [] };
const fail = (e: unknown) => setNotice(String(e));
const matches = (name: string, filter: string) => name.toLowerCase().includes(filter.trim().toLowerCase());
const date = (rfc3339: string) => new Date(rfc3339).toLocaleString();

type Selected = { kind: "builtin" | "personal"; key: string } | null;
type Pending =
  | { kind: "copy"; entry: BuiltinEntry }
  | { kind: "rename"; entry: PersonalEntry }
  | { kind: "comment"; entry: PersonalEntry };

export function LibraryPanel() {
  const { header, libraryRev } = useUiState();
  const [list, setList] = useState<LibraryList>(EMPTY);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<Selected>(null);
  const [editing, setEditing] = useState<Pending | null>(null);

  useEffect(() => {
    let live = true;
    backend.libraryList().then((l) => live && setList(l)).catch(fail);
    return () => {
      live = false;
    };
  }, [libraryRev]);

  const gridW = header?.gridWidth ?? 0;
  const gridH = header?.gridHeight ?? 0;
  const tooBig = (e: { width: number; height: number }) => e.width > gridW || e.height > gridH;

  const stats = (e: BuiltinEntry | PersonalEntry) => {
    const { period, speed } = e as Partial<BuiltinEntry>;
    return (
    <div style={{ fontSize: 12, opacity: 0.8 }}>
      {e.width} × {e.height} · {e.live} live
      {period != null && ` · period ${period}`}
      {speed && ` · ${speed}`}
    </div>
    );
  };

  const tooBigNote = (e: BuiltinEntry | PersonalEntry) =>
    tooBig(e) && (
      <div style={{ color: "#ffe9a8", fontSize: 12 }}>
        Larger than the universe ({gridW} × {gridH})
      </div>
    );

  const editingHere = (kind: Pending["kind"], e: BuiltinEntry | PersonalEntry) =>
    editing?.kind === kind && editing.entry === e ? editing : null;

  const builtin = list.builtin.filter((e) => matches(e.name, filter));
  const personal = list.personal.filter((e) => matches(e.name, filter));
  const categories = [...new Set(builtin.map((e) => e.category))];

  const row = (kind: "builtin" | "personal", key: string, source: Source, e: BuiltinEntry | PersonalEntry, details: React.ReactNode) => {
    const open = selected?.kind === kind && selected.key === key;
    const blocked = tooBig(e);
    const nameEdit = editingHere(kind === "builtin" ? "copy" : "rename", e);
    return (
      <li key={key} style={{ padding: 4, borderBottom: "1px solid #2c323c", background: open ? "#262c36" : undefined }}>
        <div style={{ display: "flex", gap: 8, cursor: blocked ? "not-allowed" : "pointer", opacity: blocked ? 0.6 : 1 }}
          aria-disabled={blocked}
          onClick={() => {
            setSelected({ kind, key });
            if (!blocked) void placeSource(source);
          }}>
          <PatternPreview source={source} />
          <div style={{ flex: 1 }}>
            <div style={{ display: "flex", gap: 4, alignItems: "flex-start", justifyContent: "space-between" }}>
              {nameEdit ? <InlineEdit kind="name" initial={nameEdit.entry.name} submit={submitFor(nameEdit)} onClose={() => setEditing(null)} /> : <strong>{e.name}</strong>}
              {kind === "builtin" && (
                <button aria-label="Copy to personal" title="Copy to personal"
                  onClick={(ev) => { ev.stopPropagation(); setEditing({ kind: "copy", entry: e as BuiltinEntry }); }}>⧉</button>
              )}
            </div>
            {stats(e)}
            {tooBigNote(e)}
          </div>
        </div>
        {open && <div style={{ marginTop: 4 }}>{details}</div>}
      </li>
    );
  };

  const submitFor = (d: Pending) => async (value: string, replace: boolean) => {
    const reply =
      d.kind === "copy" ? await backend.libraryCopyBuiltin(d.entry.id, value, replace)
      : d.kind === "rename" ? await backend.libraryRename(d.entry.name, value, replace)
      : await backend.librarySetComment(d.entry.name, value).then(() => null);
    if (!reply || !reply.nameTaken) {
      bumpLibrary();
      if (d.kind === "rename") setSelected({ kind: "personal", key: value });
    }
    return reply;
  };

  return (
    <aside aria-label="Pattern library"
      style={{ width: 300, overflowY: "auto", background: "#1b1f26", color: "#e8f1ff", borderLeft: "1px solid #2c323c", position: "relative" }}>
      <input type="search" aria-label="Filter patterns" placeholder="Filter by name" value={filter}
        onChange={(e) => setFilter(e.target.value)} style={{ margin: 8, width: "calc(100% - 16px)" }} />

      {list.problems.length > 0 && (
        <section aria-label="Problem files" style={{ margin: 8, color: "#ffe9a8" }}>
          <h4 style={{ margin: "4px 0" }}>Files that could not be used</h4>
          <ul style={{ margin: 0, paddingLeft: 16 }}>
            {list.problems.map((p) => (
              <li key={p.file}>{p.file}: {p.reason}</li>
            ))}
          </ul>
        </section>
      )}

      <h3 style={{ margin: "8px" }}>Built-in</h3>
      {categories.map((c) => (
        <section key={c} aria-label={CATEGORY_NAMES[c]}>
          <h4 style={{ margin: "4px 8px" }}>{CATEGORY_NAMES[c]}</h4>
          <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
            {builtin.filter((e) => e.category === c).map((e) =>
              row("builtin", e.id, { builtin: e.id }, e, (
                <>
                  <p style={{ margin: "4px 0" }}>{e.description}</p>
                </>
              )))}
          </ul>
        </section>
      ))}

      <h3 style={{ margin: "8px" }}>
        Personal{" "}
        <button onClick={() => void backend.libraryShowFolder().catch(fail)}>Show folder</button>
      </h3>
      {list.personal.length === 0 && (
        <p style={{ margin: 8 }}>
          No personal patterns yet. Select an area and choose Save to library, or copy a built-in pattern.
        </p>
      )}
      <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
        {personal.map((e) =>
          row("personal", e.name, { personal: e.name }, e, (
            <>
              {editingHere("comment", e)
                ? <InlineEdit kind="comment" initial={e.comment} submit={submitFor(editing!)} onClose={() => setEditing(null)} />
                : e.comment && <p style={{ margin: "4px 0", whiteSpace: "pre-wrap" }}>{e.comment}</p>}
              <div style={{ fontSize: 12, opacity: 0.8 }}>Saved {date(e.created)} · changed {date(e.changed)}</div>
              <button onClick={() => setEditing({ kind: "rename", entry: e })}>Rename</button>{" "}
              <button onClick={() => setEditing({ kind: "comment", entry: e })}>Edit comment</button>{" "}
              <button onClick={() => void backend.libraryDelete(e.name).then((gone) => gone && bumpLibrary()).catch(fail)}>Delete</button>{" "}
              <button onClick={() => void backend.exportRle({ personal: e.name }).catch(fail)}>Export</button>
            </>
          )))}
      </ul>

    </aside>
  );
}
