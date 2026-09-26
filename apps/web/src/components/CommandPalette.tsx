import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Command } from "cmdk";
import { FileText, Hash, MessagesSquare, Search } from "lucide-react";
import { useDeferredValue, useEffect, useState } from "react";
import { q } from "../api/queries";

const MAX_TOPICS = 6;

/** ⌘K / Ctrl+K: search notes by keyword, jump to a topic or tag. */
export function CommandPalette({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [input, setInput] = useState("");
  const text = useDeferredValue(input.trim());
  const navigate = useNavigate();
  const notes = useQuery({ ...q.notes({ q: text, limit: 20 }), enabled: open });
  const topics = useQuery({ ...q.topics(), enabled: open });
  const tags = useQuery({ ...q.tags(), enabled: open });

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onOpenChange(!open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onOpenChange]);

  const needle = text.toLowerCase();
  const topicHits = needle
    ? (topics.data ?? []).filter((t) => t.label.toLowerCase().includes(needle)).slice(0, MAX_TOPICS)
    : [];
  const tagHits = needle ? (tags.data ?? []).filter((t) => t.tag.includes(needle)).slice(0, 6) : [];

  const go = (action: () => void) => {
    onOpenChange(false);
    setInput("");
    action();
  };

  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      shouldFilter={false}
      label="搜索"
      overlayClassName="fixed inset-0 z-40 bg-ink/25"
      contentClassName="fixed inset-x-4 top-[12vh] z-50 mx-auto max-w-xl rounded-panel border border-line-strong bg-panel text-ink"
    >
      <div className="flex items-center gap-2 border-b border-line px-3">
        <Search className="size-4 text-ink-faint" aria-hidden />
        <Command.Input
          value={input}
          onValueChange={setInput}
          placeholder="搜索 note、topic、tag…"
          className="h-12 w-full bg-transparent text-base outline-none placeholder:text-ink-faint"
        />
      </div>
      <Command.List className="max-h-[60vh] overflow-y-auto p-1.5">
        <Command.Empty className="px-3 py-6 text-center text-sm text-ink-faint">
          {notes.isFetching ? "搜索中…" : "没有匹配的 note"}
        </Command.Empty>
        {topicHits.length > 0 && (
          <Command.Group heading="Topic" className={GROUP}>
            {topicHits.map((t) => (
              <Command.Item
                key={t.topic_id}
                value={`topic:${t.topic_id}`}
                className={ITEM}
                onSelect={() =>
                  go(() => navigate({ to: "/topics/$topicId", params: { topicId: t.topic_id } }))
                }
              >
                <MessagesSquare className="size-4 shrink-0 text-ink-faint" aria-hidden />
                <span className="truncate">{t.label}</span>
                <span className="ml-auto text-xs text-ink-faint">{t.ask_count} 次</span>
              </Command.Item>
            ))}
          </Command.Group>
        )}
        {tagHits.length > 0 && (
          <Command.Group heading="Tag" className={GROUP}>
            {tagHits.map((t) => (
              <Command.Item
                key={t.tag}
                value={`tag:${t.tag}`}
                className={ITEM}
                onSelect={() => go(() => navigate({ to: "/", search: { tag: t.tag } }))}
              >
                <Hash className="size-4 shrink-0 text-ink-faint" aria-hidden />
                {t.tag}
                <span className="ml-auto text-xs text-ink-faint">{t.notes}</span>
              </Command.Item>
            ))}
          </Command.Group>
        )}
        {(notes.data ?? []).length > 0 && (
          <Command.Group heading={text ? "Note" : "最近的 note"} className={GROUP}>
            {(notes.data ?? []).map((n) => (
              <Command.Item
                key={n.id}
                value={`note:${n.id}`}
                className={ITEM}
                onSelect={() =>
                  go(() => navigate({ to: "/notes/$noteId", params: { noteId: n.id } }))
                }
              >
                <FileText className="size-4 shrink-0 text-ink-faint" aria-hidden />
                <span className="truncate">{n.title}</span>
              </Command.Item>
            ))}
          </Command.Group>
        )}
      </Command.List>
    </Command.Dialog>
  );
}

const GROUP =
  "[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:pt-2 [&_[cmdk-group-heading]]:pb-1 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wider [&_[cmdk-group-heading]]:text-ink-faint";
const ITEM =
  "flex cursor-pointer items-center gap-2 rounded-[1px] px-2 py-2 text-sm data-[selected=true]:bg-brand-soft data-[selected=true]:text-brand-ink";
