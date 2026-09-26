import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Link, useSearch } from "@tanstack/react-router";
import { ArrowRight, ChevronDown, MessageSquareText, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { TimelineItem } from "../api/generated/TimelineItem";
import { q } from "../api/queries";
import { Markdown } from "../components/Markdown";
import { AskCount, Button, cx, Empty, ErrorBox, Loading, TagChip } from "../components/ui";
import {
  agentLabel,
  dayLabel,
  excerpt,
  formatTime,
  localDay,
  projectName,
  stripTitle,
  withoutSection,
} from "../lib/format";

/** Every distilled note, newest first, grouped by day. */
export function DistillPage() {
  const { tag } = useSearch({ from: "/" });
  const timeline = useInfiniteQuery(q.timeline(tag));
  const items = timeline.data?.pages.flatMap((p) => p.items) ?? [];

  return (
    <div className="mx-auto max-w-[48rem]">
      <header className="mb-8">
        <h1 className="text-[1.875rem] leading-tight sm:text-4xl">{tag ? `#${tag}` : "Distill"}</h1>
        <p className="mt-2 flex flex-wrap items-center gap-3 text-sm text-ink-faint">
          {tag ? "这个 tag 下的 note，最新的在前。" : "你 distill 过的每个问题，最新的在前。"}
          {tag && (
            <Link to="/" className="inline-flex items-center gap-1 text-brand-ink hover:underline">
              <X className="size-3.5" aria-hidden />
              看全部
            </Link>
          )}
        </p>
      </header>
      {timeline.isPending ? (
        <Loading />
      ) : timeline.isError ? (
        <ErrorBox error={timeline.error} />
      ) : items.length === 0 ? (
        <Empty>
          {tag
            ? "这个 tag 下没有 note。"
            : "还没有 note。在 Codex 或 Claude Code 里聊完一个值得记住的问题后，说“distill 一下”。"}
        </Empty>
      ) : (
        <>
          <Timeline items={items} />
          <MorePages
            hasMore={timeline.hasNextPage}
            loading={timeline.isFetchingNextPage}
            onMore={() => void timeline.fetchNextPage()}
          />
        </>
      )}
    </div>
  );
}

function Timeline({ items }: { items: TimelineItem[] }) {
  const days: { day: string; items: TimelineItem[] }[] = [];
  for (const item of items) {
    const day = localDay(new Date(item.created));
    const last = days.at(-1);
    if (last?.day === day) {
      last.items.push(item);
    } else {
      days.push({ day, items: [item] });
    }
  }
  return (
    <div className="space-y-10">
      {days.map(({ day, items }) => (
        <section key={day} aria-label={dayLabel(day)}>
          <h2 className="mb-4 font-body text-sm text-ink-faint">{dayLabel(day)}</h2>
          <ol className="relative ml-1.5 border-l border-line-strong">
            {items.map((item) => (
              <li key={item.id} className="relative pb-6 pl-6 last:pb-0">
                <span
                  className={cx(
                    "absolute top-[1.15rem] -left-[5px] size-[9px] border",
                    item.ask_index >= 2 ? "border-warning bg-warning" : "border-brand bg-panel",
                  )}
                  aria-hidden
                />
                <Entry item={item} />
              </li>
            ))}
          </ol>
        </section>
      ))}
    </div>
  );
}

/**
 * The question and the first two lines of what was distilled. Activating it loads and
 * shows the whole note in place.
 */
function Entry({ item }: { item: TimelineItem }) {
  const [open, setOpen] = useState(false);
  const bodyId = `entry-${item.id}`;
  return (
    <article
      className={cx(
        "rounded-panel border transition-colors duration-150",
        open ? "border-line-strong bg-panel" : "border-transparent hover:bg-panel-hover",
      )}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen(!open)}
        className="block w-full px-3 py-3 text-left"
      >
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-faint">
          <time dateTime={item.created} className="tabular-nums">
            {formatTime(item.created)}
          </time>
          <span>{agentLabel[item.source.agent]}</span>
          <span className="min-w-0 truncate">{projectName(item.source)}</span>
          {item.ask_count >= 2 && (
            <span className="rounded-[1px] bg-warning-soft px-1.5 py-px text-warning">
              第 {item.ask_index} 次问 · 共 {item.ask_count} 次
            </span>
          )}
          <ChevronDown
            className={cx("ml-auto size-4 transition-transform duration-150", open && "rotate-180")}
            aria-hidden
          />
        </div>
        <h3 className="mt-1.5 text-lg leading-snug">{item.title}</h3>
        <p className="mt-2 flex gap-2 text-[0.9375rem] text-ink">
          <span className="mt-0.5 shrink-0 text-xs text-ink-faint">问</span>
          <span className="line-clamp-3">{item.question}</span>
        </p>
        {!open && (
          <p className="mt-1.5 flex gap-2 text-[0.9375rem] text-ink-muted">
            <span className="mt-0.5 shrink-0 text-xs text-brand-ink">答</span>
            <span className="line-clamp-2">{excerpt(item.conclusion)}</span>
          </p>
        )}
      </button>
      <div id={bodyId} hidden={!open} className="px-3 pb-4">
        {open && <FullNote item={item} />}
      </div>
      {item.tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-3 pb-3">
          {item.tags.map((tag) => (
            <TagChip key={tag} tag={tag} />
          ))}
        </div>
      )}
    </article>
  );
}

function FullNote({ item }: { item: TimelineItem }) {
  const note = useQuery(q.note(item.id));
  if (note.isPending) {
    return <Loading />;
  }
  if (note.isError) {
    return <ErrorBox error={note.error} />;
  }
  return (
    <div className="border-t border-line pt-2">
      <Markdown source={withoutSection(stripTitle(note.data.markdown), "问题")} />
      <div className="mt-5 flex flex-wrap items-center gap-x-5 gap-y-2 text-sm">
        <Link
          to="/notes/$noteId"
          params={{ noteId: item.id }}
          className="inline-flex items-center gap-1 text-brand-ink hover:underline"
        >
          打开 note 页
          <ArrowRight className="size-3.5" aria-hidden />
        </Link>
        <Link
          to="/topics/$topicId"
          params={{ topicId: item.topic_id }}
          className="inline-flex min-w-0 items-center gap-2 text-ink-muted hover:text-brand-ink"
        >
          <span className="truncate">Topic：{item.topic_label}</span>
          <AskCount count={item.ask_count} />
        </Link>
        <span className="inline-flex items-center gap-1 text-ink-faint">
          <MessageSquareText className="size-3.5" aria-hidden />
          {item.annotations > 0 ? `${item.annotations} 条 annotation` : "还没有 annotation"}
        </span>
      </div>
    </div>
  );
}

/** Loads the next page when the end of the list scrolls into view, or on click. */
function MorePages({
  hasMore,
  loading,
  onMore,
}: {
  hasMore: boolean;
  loading: boolean;
  onMore: () => void;
}) {
  const sentinel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = sentinel.current;
    if (!el || !hasMore) {
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting) && !loading) {
        onMore();
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [hasMore, loading, onMore]);

  if (!hasMore) {
    return null;
  }
  return (
    <div ref={sentinel} className="mt-8 flex justify-center">
      <Button disabled={loading} onClick={onMore}>
        {loading ? "载入中…" : "更早的 note"}
      </Button>
    </div>
  );
}
