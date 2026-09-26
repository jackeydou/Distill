import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import type { TopicCount } from "../api/generated/TopicCount";
import { q } from "../api/queries";
import { DuplicateList } from "../components/Duplicates";
import { TopicRows } from "../components/TopicRows";
import { cx, ErrorBox, Loading, PageHeader } from "../components/ui";

export type TopicView = "asked" | "recent" | "undigested" | "duplicates";

const VIEWS: { value: TopicView; label: string }[] = [
  { value: "asked", label: "问得最多" },
  { value: "recent", label: "最近问过" },
  { value: "undigested", label: "重问过、没有 annotation" },
  { value: "duplicates", label: "可能重复" },
];

function arrange(topics: TopicCount[], view: TopicView): TopicCount[] {
  switch (view) {
    case "asked":
      return topics;
    case "recent":
      return [...topics].sort((a, b) => b.last_asked.localeCompare(a.last_asked));
    case "undigested":
    case "duplicates":
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
      {view === "duplicates" ? (
        <>
          <p className="mb-4 text-sm text-ink-faint">
            同一个问题被存成了不同 topic 时，提问次数会被拆散。合并后次数加在一起，note 文件不变。
          </p>
          <DuplicateList />
        </>
      ) : topics.isPending ? (
        <Loading />
      ) : topics.isError ? (
        <ErrorBox error={topics.error} />
      ) : (
        <TopicRows topics={arrange(topics.data, view)} />
      )}
    </div>
  );
}
