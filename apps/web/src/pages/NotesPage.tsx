import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { useDeferredValue, useState } from "react";
import { q } from "../api/queries";
import { NoteList } from "../components/NoteList";
import { ErrorBox, Loading, PageHeader } from "../components/ui";

const LIMIT = 500;

export function NotesPage() {
  const search = useSearch({ from: "/notes" });
  const navigate = useNavigate({ from: "/notes" });
  const [input, setInput] = useState(search.q ?? "");
  const text = useDeferredValue(input.trim());
  const notes = useQuery(q.notes({ q: text, limit: LIMIT }));

  return (
    <div>
      <PageHeader title="Notes" />
      <label className="mb-6 flex items-center gap-2 rounded-panel border border-line-strong bg-panel px-3 focus-within:border-brand">
        <Search className="size-4 text-ink-faint" aria-hidden />
        <input
          value={input}
          onChange={(event) => {
            setInput(event.target.value);
            void navigate({ search: { q: event.target.value || undefined }, replace: true });
          }}
          placeholder="关键词：标题、问题、正文"
          aria-label="搜索 note"
          className="h-10 w-full bg-transparent text-ink outline-none placeholder:text-ink-faint"
        />
      </label>
      {notes.isPending ? (
        <Loading />
      ) : notes.isError ? (
        <ErrorBox error={notes.error} />
      ) : (
        <>
          <p className="mb-3 text-sm text-ink-faint">
            {text ? `${notes.data.length} 条匹配` : `最近的 ${notes.data.length} 条`}
          </p>
          <NoteList notes={notes.data} empty={text ? "没有匹配的 note。" : "还没有 note。"} />
        </>
      )}
    </div>
  );
}
