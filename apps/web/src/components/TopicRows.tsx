import { Link } from "@tanstack/react-router";
import type { TopicCount } from "../api/generated/TopicCount";
import { formatDate } from "../lib/format";
import { AskCount, Empty } from "./ui";

export function TopicRows({ topics }: { topics: TopicCount[] }) {
  if (topics.length === 0) {
    return <Empty>没有 topic。</Empty>;
  }
  return (
    <ul className="border-t border-line">
      {topics.map((t) => (
        <li key={t.topic_id} className="border-b border-line">
          <Link
            to="/topics/$topicId"
            params={{ topicId: t.topic_id }}
            className="group flex items-baseline gap-3 px-1 py-2.5 transition-colors duration-150 hover:bg-panel-hover sm:px-3"
          >
            <span className="min-w-0 flex-1 truncate text-ink group-hover:text-brand-ink">
              {t.label}
            </span>
            {!t.annotated && t.ask_count >= 2 && (
              <span className="hidden shrink-0 text-xs text-ink-faint sm:inline">
                无 annotation
              </span>
            )}
            <span className="hidden shrink-0 text-xs text-ink-faint tabular-nums sm:inline">
              {formatDate(t.last_asked)}
            </span>
            <AskCount count={t.ask_count} />
          </Link>
        </li>
      ))}
    </ul>
  );
}
