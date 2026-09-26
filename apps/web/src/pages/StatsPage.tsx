import { useQuery } from "@tanstack/react-query";
import { q } from "../api/queries";
import { BarList, Columns } from "../components/charts";
import { TopicRows } from "../components/TopicRows";
import { Empty, ErrorBox, Loading, PageHeader, Section, Stat } from "../components/ui";
import { weekKey, weeksEnding } from "../lib/weeks";

const WEEKS = 16;
const TOP_BARS = 12;

export function StatsPage() {
  const stats = useQuery(q.stats());
  if (stats.isPending) {
    return <Loading />;
  }
  if (stats.isError) {
    return <ErrorBox error={stats.error} />;
  }
  const s = stats.data;
  const perWeek = new Map(s.by_week.map((w) => [w.week, w.notes]));
  const columns = weeksEnding(weekKey(new Date()), WEEKS).map(({ key, monday }) => ({
    key,
    label: `${monday.getUTCMonth() + 1}/${monday.getUTCDate()}`,
    value: perWeek.get(key) ?? 0,
  }));
  const undigested = s.top_topics.filter((t) => t.ask_count >= 2 && !t.annotated);

  return (
    <div>
      <PageHeader title="统计" />
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label="Note" value={s.notes} />
        <Stat label="Topic" value={s.topics} />
        <Stat label="重问过的 topic" value={s.repeated_topics} hint="问过 2 次及以上" />
        <Stat label="Annotation" value={s.annotations} />
      </div>
      <Section title={`每周 distill 的 note（最近 ${WEEKS} 周）`}>
        <Columns columns={columns} unit="条" />
      </Section>
      <Section title="问得最多的 topic">
        <TopicRows topics={s.top_topics.filter((t) => t.ask_count >= 2)} />
      </Section>
      <Section title="重问过、还没写 annotation 的 topic">
        {undigested.length > 0 ? (
          <TopicRows topics={undigested} />
        ) : (
          <Empty>重问过的 topic 都写了 annotation。</Empty>
        )}
      </Section>
      <div className="grid gap-x-10 md:grid-cols-2">
        <Section title="按 tag">
          <BarList
            bars={s.tags.slice(0, TOP_BARS).map((t) => ({
              key: t.tag,
              label: `#${t.tag}`,
              value: t.notes,
            }))}
            link={(b) => ({ to: "/tags/$tag", params: { tag: b.key } })}
          />
        </Section>
        <Section title="按项目">
          <BarList
            bars={s.by_project.slice(0, TOP_BARS).map((p) => ({
              key: p.project,
              label: p.project.split("/").slice(-2).join("/"),
              value: p.notes,
            }))}
          />
        </Section>
      </div>
    </div>
  );
}
