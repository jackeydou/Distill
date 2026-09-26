import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowRight, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import type { Stats } from "../api/generated/Stats";
import { q } from "../api/queries";
import { BarList, Columns, type Day, Heatmap } from "../components/charts";
import { DuplicateList } from "../components/Duplicates";
import { TopicRows } from "../components/TopicRows";
import { Empty, ErrorBox, Loading, PageHeader, Stat } from "../components/ui";
import { agentLabel, localDay, projectName } from "../lib/format";
import { calendarWeeks, weekKey, weeksEnding } from "../lib/weeks";

const HEATMAP_WEEKS = 26;
const COLUMN_WEEKS = 16;
const ACTIVE_WINDOW_DAYS = 30;
const TOP = 8;
const BARS = 10;

const dayTitle = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "long",
  day: "numeric",
  weekday: "short",
});

/** How the user has been learning: activity over time, repeats, and where it happens. */
export function ReviewPage() {
  const stats = useQuery(q.stats());
  const problems = useQuery(q.problems());
  if (stats.isPending) {
    return <Loading />;
  }
  if (stats.isError) {
    return <ErrorBox error={stats.error} />;
  }
  const s = stats.data;
  const problemCount =
    (problems.data?.conflicts.length ?? 0) + (problems.data?.invalid_files.length ?? 0);
  const repeated = s.top_topics.filter((t) => t.ask_count >= 2);
  const undigested = repeated.filter((t) => !t.annotated);

  return (
    <div>
      <PageHeader title="Review">
        {s.notes} 条 note，{s.topics} 个 topic，{s.annotations} 条 annotation。
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
      <Metrics stats={s} />

      <Card title={`活动（最近 ${HEATMAP_WEEKS} 周）`}>
        <Heatmap weeks={heatmap(s)} unit="条 note" />
      </Card>

      <Card title={`每周 distill 的 note（最近 ${COLUMN_WEEKS} 周）`}>
        <Columns columns={weekly(s)} unit="条" />
      </Card>

      <div className="grid gap-x-6 lg:grid-cols-2">
        <Card title="问得最多的 topic">
          {repeated.length > 0 ? (
            <TopicRows compact topics={repeated.slice(0, TOP)} />
          ) : (
            <Empty>还没有问过第二次的问题。</Empty>
          )}
        </Card>
        <Card title="重问过、还没写 annotation">
          {undigested.length > 0 ? (
            <TopicRows compact topics={undigested.slice(0, TOP)} />
          ) : (
            <Empty>重问过的 topic 都写了 annotation。</Empty>
          )}
        </Card>
      </div>

      <Card title="可能是同一个问题的 topic">
        <DuplicateList />
      </Card>

      <div className="grid gap-x-6 lg:grid-cols-2">
        <Card title="按 tag">
          <BarList
            tagLinks
            bars={s.tags.slice(0, BARS).map((t) => ({
              key: t.tag,
              label: `#${t.tag}`,
              value: t.notes,
            }))}
          />
        </Card>
        <Card title="按项目">
          <BarList
            bars={s.by_project.slice(0, BARS).map((p) => ({
              key: p.project,
              label: projectName({ cwd: p.project }),
              value: p.notes,
            }))}
          />
        </Card>
        <Card title="按 agent">
          <BarList
            bars={s.by_agent.map((a) => ({
              key: a.agent,
              label: agentLabel[a.agent],
              value: a.notes,
            }))}
          />
        </Card>
      </div>
    </div>
  );
}

function Metrics({ stats: s }: { stats: Stats }) {
  const now = new Date();
  const perWeek = new Map(s.by_week.map((w) => [w.week, w.notes]));
  const [lastWeek, thisWeek] = weeksEnding(weekKey(now), 2);
  const current = perWeek.get(thisWeek?.key ?? "") ?? 0;
  const previous = perWeek.get(lastWeek?.key ?? "") ?? 0;

  const since = new Date(now);
  since.setDate(now.getDate() - (ACTIVE_WINDOW_DAYS - 1));
  const activeDays = s.by_day.filter((d) => d.day >= localDay(since)).length;

  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <Stat
        label="本周 distill"
        value={current}
        hint={`上周 ${previous}${delta(current, previous)}`}
      />
      <Stat
        label="重问过的 topic"
        value={s.repeated_topics}
        hint={`占全部 topic 的 ${percent(s.repeated_topics, s.topics)}`}
      />
      <Stat
        label="annotation 覆盖"
        value={percent(s.annotated_repeated_topics, s.repeated_topics)}
        hint={`重问过的 ${s.repeated_topics} 个 topic 中写了 ${s.annotated_repeated_topics} 个`}
      />
      <Stat
        label={`近 ${ACTIVE_WINDOW_DAYS} 天活跃`}
        value={`${activeDays} 天`}
        hint="有 distill 的天数"
      />
    </div>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mt-6 rounded-panel border border-line bg-panel p-4 sm:p-5">
      <h2 className="mb-4 text-lg">{title}</h2>
      {children}
    </section>
  );
}

function heatmap(s: Stats): Day[][] {
  const perDay = new Map(s.by_day.map((d) => [d.day, d.notes]));
  const today = localDay(new Date());
  return calendarWeeks(HEATMAP_WEEKS).map((week) =>
    week.map((date) => {
      const day = localDay(date);
      return {
        day,
        label: dayTitle.format(date),
        value: perDay.get(day) ?? 0,
        future: day > today,
      };
    }),
  );
}

function weekly(s: Stats) {
  const perWeek = new Map(s.by_week.map((w) => [w.week, w.notes]));
  return weeksEnding(weekKey(new Date()), COLUMN_WEEKS).map(({ key, monday }) => ({
    key,
    label: `${monday.getUTCMonth() + 1}/${monday.getUTCDate()}`,
    value: perWeek.get(key) ?? 0,
  }));
}

function percent(part: number, whole: number): string {
  return whole === 0 ? "—" : `${Math.round((part / whole) * 100)}%`;
}

function delta(current: number, previous: number): string {
  if (current === previous) {
    return "，持平";
  }
  return current > previous ? `，多 ${current - previous}` : `，少 ${previous - current}`;
}
