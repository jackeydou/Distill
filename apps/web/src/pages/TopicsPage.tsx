import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import type { TopicCount } from "../api/generated/TopicCount";
import { q } from "../api/queries";
import { TopicRows } from "../components/TopicRows";
import { cx, ErrorBox, Loading, PageHeader } from "../components/ui";

export type TopicView = "asked" | "recent" | "undigested";

const VIEWS: { value: TopicView; label: string }[] = [
  { value: "asked", label: "问得最多" },
  { value: "recent", label: "最近问过" },
  { value: "undigested", label: "重问过、没有 annotation" },
];

function arrange(topics: TopicCount[], view: TopicView): TopicCount[] {
  switch (view) {
    case "asked":
      return topics;
    case "recent":
      return [...topics].sort((a, b) => b.last_asked.localeCompare(a.last_asked));
    case "undigested":
      return topics.filter((t) => t.ask_count >= 2 && !t.annotated);
  }
}

export function TopicsPage() {
  const { view = "asked" } = useSearch({ from: "/topics" });
  const navigate = useNavigate({ from: "/topics" });
  const topics = useQuery(q.topics());

  return (
    <div>
      <PageHeader title="Topics">
        一个 topic 是一个问题。同一个问题问了几次，就有几条 note。
      </PageHeader>
      <div className="mb-5 flex flex-wrap gap-1" role="tablist" aria-label="排序">
        {VIEWS.map((v) => (
          <button
            key={v.value}
            type="button"
            role="tab"
            aria-selected={view === v.value}
            onClick={() => navigate({ search: { view: v.value }, replace: true })}
            className={cx(
              "rounded-[1px] px-2.5 py-1 text-sm transition-colors duration-150",
              view === v.value
                ? "bg-brand-strong text-on-brand"
                : "text-ink-muted hover:bg-panel-hover hover:text-ink",
            )}
          >
            {v.label}
          </button>
        ))}
      </div>
      {topics.isPending ? (
        <Loading />
      ) : topics.isError ? (
        <ErrorBox error={topics.error} />
      ) : (
        <TopicRows topics={arrange(topics.data, view)} />
      )}
    </div>
  );
}
