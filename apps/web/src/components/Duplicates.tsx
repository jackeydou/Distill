import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { GitMerge, X } from "lucide-react";
import { api } from "../api/client";
import type { TopicPair } from "../api/generated/TopicPair";
import { q, useEdit } from "../api/queries";
import { Button, Empty, ErrorBox, Loading } from "./ui";

/** Topic pairs the embedding model thinks are one question, each with merge and dismiss. */
export function DuplicateList() {
  const dup = useQuery(q.duplicates());
  if (dup.isPending) {
    return <Loading />;
  }
  if (dup.isError) {
    return <ErrorBox error={dup.error} />;
  }
  if (!dup.data.model_installed) {
    return (
      <Empty>
        找重复 topic 需要本机的 embedding 模型。在终端运行{" "}
        <code className="font-mono">distill model pull</code>（约 0.25 GB）。
      </Empty>
    );
  }
  if (dup.data.pairs.length === 0) {
    return <Empty>没有看起来是同一个问题的 topic。</Empty>;
  }
  return (
    <ul className="space-y-3">
      {dup.data.pairs.map((pair) => (
        <PairRow key={`${pair.a.topic_id}:${pair.b.topic_id}`} pair={pair} />
      ))}
    </ul>
  );
}

function PairRow({ pair }: { pair: TopicPair }) {
  const merge = useEdit(() => api.mergeTopic(pair.a.topic_id, { into: pair.b.topic_id }));
  const dismiss = useEdit(() => api.dismissDuplicate({ a: pair.a.topic_id, b: pair.b.topic_id }));
  const busy = merge.isPending || dismiss.isPending;
  return (
    <li className="rounded-panel border border-line bg-panel p-4">
      <div className="text-xs text-ink-faint">相似度 {Math.round(pair.similarity * 100)}%</div>
      <div className="mt-2 grid gap-1 text-sm sm:grid-cols-[1fr_auto_1fr] sm:items-center sm:gap-3">
        <TopicLink id={pair.a.topic_id} label={pair.a.label} count={pair.a_count} />
        <span className="text-ink-faint" aria-hidden>
          →
        </span>
        <TopicLink id={pair.b.topic_id} label={pair.b.label} count={pair.b_count} />
      </div>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button variant="primary" disabled={busy} onClick={() => merge.mutate(undefined)}>
          <GitMerge className="size-4" aria-hidden />
          合并进右边
        </Button>
        <Button disabled={busy} onClick={() => dismiss.mutate(undefined)}>
          <X className="size-4" aria-hidden />
          不是同一个问题
        </Button>
      </div>
      {(merge.error || dismiss.error) && (
        <div className="mt-3">
          <ErrorBox error={merge.error ?? dismiss.error} />
        </div>
      )}
    </li>
  );
}

function TopicLink({ id, label, count }: { id: string; label: string; count: number }) {
  return (
    <Link
      to="/topics/$topicId"
      params={{ topicId: id }}
      className="min-w-0 text-ink hover:text-brand-ink"
    >
      <span className="line-clamp-2">{label}</span>
      <span className="text-xs text-ink-faint">{count} 条 note</span>
    </Link>
  );
}
