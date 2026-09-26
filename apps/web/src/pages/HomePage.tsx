import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowRight, TriangleAlert } from "lucide-react";
import { q } from "../api/queries";
import { NoteList } from "../components/NoteList";
import { TopicRows } from "../components/TopicRows";
import { Empty, ErrorBox, Loading, PageHeader, Section } from "../components/ui";

const RECENT = 8;
const TOP = 6;

export function HomePage() {
  const stats = useQuery(q.stats());
  const recent = useQuery(q.notes({ limit: RECENT }));
  const problems = useQuery(q.problems());

  if (stats.isPending || recent.isPending) {
    return <Loading />;
  }
  if (stats.isError) {
    return <ErrorBox error={stats.error} />;
  }
  if (recent.isError) {
    return <ErrorBox error={recent.error} />;
  }
  const s = stats.data;
  const repeated = s.top_topics.filter((t) => t.ask_count >= 2);
  const undigested = repeated.filter((t) => !t.annotated);
  const problemCount =
    (problems.data?.conflicts.length ?? 0) + (problems.data?.invalid_files.length ?? 0);

  if (s.notes === 0) {
    return (
      <>
        <PageHeader title="Distill" />
        <Empty>
          还没有 note。在 Codex 或 Claude Code 里聊完一个值得记住的问题后，说“distill 一下”。
        </Empty>
      </>
    );
  }
  return (
    <div>
      <PageHeader title="Distill">
        {s.notes} 条 note，{s.topics} 个 topic，其中 {s.repeated_topics} 个问过不止一次。
      </PageHeader>
      {problemCount > 0 && (
        <Link
          to="/problems"
          className="mb-8 flex items-center gap-3 rounded-panel border border-error/40 bg-error-soft px-4 py-3 text-sm text-error"
        >
          <TriangleAlert className="size-4 shrink-0" aria-hidden />有 {problemCount}{" "}
          个文件需要处理：同步冲突或读不了的文件。
          <ArrowRight className="ml-auto size-4" aria-hidden />
        </Link>
      )}
      <Section title="问得最多" action={<More to="/topics" />}>
        {repeated.length > 0 ? (
          <TopicRows topics={repeated.slice(0, TOP)} />
        ) : (
          <Empty>还没有问过第二次的问题。</Empty>
        )}
      </Section>
      {undigested.length > 0 && (
        <Section title="重问过、还没写 annotation">
          <p className="mb-3 text-sm text-ink-faint">
            问了不止一次却没留下自己的理解，这些最可能还没真正弄懂。
          </p>
          <TopicRows topics={undigested.slice(0, TOP)} />
        </Section>
      )}
      <Section title="最近的 note" action={<More to="/notes" />}>
        <NoteList notes={recent.data} empty="还没有 note。" />
      </Section>
    </div>
  );
}

function More({ to }: { to: string }) {
  return (
    <Link to={to} className="text-sm text-brand-ink hover:underline">
      全部 →
    </Link>
  );
}
