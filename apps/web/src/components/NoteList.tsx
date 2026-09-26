import { Link } from "@tanstack/react-router";
import { WindowVirtualizer } from "virtua";
import type { NoteHit } from "../api/generated/NoteHit";
import { excerpt, formatDate } from "../lib/format";
import { Empty, TagChip } from "./ui";

/** Notes as rows; long lists render only what is on screen. */
export function NoteList({ notes, empty }: { notes: NoteHit[]; empty: string }) {
  if (notes.length === 0) {
    return <Empty>{empty}</Empty>;
  }
  return (
    <ol className="border-t border-line">
      <WindowVirtualizer>
        {notes.map((note) => (
          <li key={note.id} className="border-b border-line">
            <NoteRow note={note} />
          </li>
        ))}
      </WindowVirtualizer>
    </ol>
  );
}

function NoteRow({ note }: { note: NoteHit }) {
  return (
    <div className="group relative px-1 py-3 transition-colors duration-150 hover:bg-panel-hover sm:px-3">
      <div className="flex items-baseline justify-between gap-4">
        <Link
          to="/notes/$noteId"
          params={{ noteId: note.id }}
          className="font-display text-[1.0625rem] leading-snug text-ink after:absolute after:inset-0 group-hover:text-brand-ink"
        >
          {note.title}
        </Link>
        <time className="shrink-0 text-xs text-ink-faint tabular-nums" dateTime={note.created}>
          {formatDate(note.created)}
        </time>
      </div>
      <p className="mt-1 line-clamp-2 text-sm text-ink-muted">{excerpt(note.conclusion)}</p>
      {note.tags.length > 0 && (
        <div className="relative z-10 mt-2 flex flex-wrap gap-1.5">
          {note.tags.map((tag) => (
            <TagChip key={tag} tag={tag} />
          ))}
        </div>
      )}
    </div>
  );
}
