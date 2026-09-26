import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { GitMerge, Pencil } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../api/client";
import type { TopicDetail } from "../api/generated/TopicDetail";
import { q, useEdit } from "../api/queries";
import { TopicPicker } from "../components/TopicPicker";
import {
  AskCount,
  Button,
  ErrorBox,
  Loading,
  PageHeader,
  Section,
  TagChip,
} from "../components/ui";
import { excerpt, formatDate } from "../lib/format";

export function TopicPage() {
  const { topicId } = useParams({ from: "/topics/$topicId" });
  const topic = useQuery(q.topic(topicId));
  const navigate = useNavigate();

  // A merged-away topic resolves to the one it was merged into; show that one's URL.
  const resolved = topic.data?.topic_id;
  useEffect(() => {
    if (resolved && resolved !== topicId) {
      void navigate({ to: "/topics/$topicId", params: { topicId: resolved }, replace: true });
    }
  }, [resolved, topicId, navigate]);

  if (topic.isPending) {
    return <Loading />;
  }
  if (topic.isError) {
    return <ErrorBox error={topic.error} />;
  }
  const t = topic.data;
  return (
    <div>
      <Label topic={t} />
      <Section title="时间线">
        <Timeline topic={t} />
      </Section>
      {t.merged_from.length > 0 && (
        <Section title="已合并进来的 topic">
          <ul className="space-y-1 text-sm">
            {t.merged_from.map((m) => (
              <li key={m.topic_id} className="text-ink-muted">
                {m.label} <span className="font-mono text-xs text-ink-faint">{m.topic_id}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}
      <Similar topic={t} />
      <Section title="合并">
        <Merge topic={t} />
      </Section>
    </div>
  );
}

function Label({ topic }: { topic: TopicDetail }) {
  const [editing, setEditing] = useState(false);
  const [label, setLabel] = useState(topic.label);
  const rename = useEdit((next: string) => api.renameTopic(topic.topic_id, { label: next }));
  const first = topic.notes[0];
  const last = topic.notes.at(-1);

  return (
    <PageHeader
      eyebrow={
        <span className="flex items-center gap-2">
          Topic <AskCount count={topic.ask_count} />
        </span>
      }
      title={
        editing ? (
          <form
            className="flex flex-col gap-2 sm:flex-row sm:items-center"
            onSubmit={(event) => {
              event.preventDefault();
              rename.mutate(label, { onSuccess: () => setEditing(false) });
            }}
          >
            <input
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              aria-label="Topic 名称"
              className="min-w-0 flex-1 rounded-panel border border-line-strong bg-panel px-2 py-1 font-display text-2xl text-ink focus:border-brand focus:outline-none"
            />
            <div className="flex gap-2">
              <Button type="submit" variant="primary" disabled={rename.isPending || !label.trim()}>
                保存
              </Button>
              <Button
                onClick={() => {
                  setLabel(topic.label);
                  setEditing(false);
                }}
              >
                取消
              </Button>
            </div>
          </form>
        ) : (
          <span className="group">
            {topic.label}
            <button
              type="button"
              aria-label="改名"
              title="改名"
              onClick={() => setEditing(true)}
              className="ml-2 inline-flex size-8 translate-y-[-2px] items-center justify-center rounded-[1px] align-middle text-ink-faint transition-colors duration-150 hover:bg-panel-hover hover:text-ink"
            >
              <Pencil className="size-4" />
            </button>
          </span>
        )
      }
    >
      {rename.error && <ErrorBox error={rename.error} />}
      {first && last && (
        <p className="text-sm text-ink-faint">
          {topic.sessions} 个会话 · 第一次 {formatDate(first.created)} · 最近{" "}
          {formatDate(last.created)}
        </p>
      )}
    </PageHeader>
  );
}

function Timeline({ topic }: { topic: TopicDetail }) {
  return (
    <ol className="relative ml-1.5 border-l border-line-strong">
      {topic.notes.map((note, i) => (
        <li key={note.id} className="relative pb-7 pl-6 last:pb-0">
          <span
            className="absolute top-2 -left-[5px] size-[9px] border border-brand bg-panel"
            aria-hidden
          />
          <div className="text-xs text-ink-faint">
            第 {i + 1} 次 · <time dateTime={note.created}>{formatDate(note.created)}</time>
            {note.annotations.length > 0 && ` · ${note.annotations.length} 条 annotation`}
          </div>
          <Link
            to="/notes/$noteId"
            params={{ noteId: note.id }}
            className="mt-1 block font-display text-lg leading-snug text-ink hover:text-brand-ink"
          >
            {note.title}
          </Link>
          <p className="mt-1 line-clamp-3 text-sm text-ink-muted">{excerpt(note.conclusion)}</p>
          {note.tags.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {note.tags.map((tag) => (
                <TagChip key={tag} tag={tag} />
              ))}
            </div>
          )}
        </li>
      ))}
    </ol>
  );
}

function Merge({ topic }: { topic: TopicDetail }) {
  const [picking, setPicking] = useState(false);
  const navigate = useNavigate();
  const merge = useEdit((into: string) => api.mergeTopic(topic.topic_id, { into }));

  return (
    <div className="space-y-3">
      <p className="text-sm text-ink-faint">
        同一个问题被存成了两个 topic 时，把这个 topic 合并进另一个：提问次数合在一起，note
        文件不变，只在这个 topic 的文件里记下合并去向。
      </p>
      {picking ? (
        <TopicPicker
          exclude={topic.topic_id}
          onCancel={() => setPicking(false)}
          onPick={(target) => {
            if (
              window.confirm(
                `把「${topic.label}」合并进「${target.label}」？合并后这个 topic 不再单独出现。`,
              )
            ) {
              merge.mutate(target.topic_id, {
                onSuccess: (merged) =>
                  navigate({ to: "/topics/$topicId", params: { topicId: merged.topic_id } }),
              });
            }
          }}
        />
      ) : (
        <Button onClick={() => setPicking(true)}>
          <GitMerge className="size-4" aria-hidden />
          合并进另一个 topic…
        </Button>
      )}
      {merge.error && <ErrorBox error={merge.error} />}
    </div>
  );
}

function Similar({ topic }: { topic: TopicDetail }) {
  const session = useQuery(q.session());
  const similar = useQuery({
    ...q.similarTopics(topic.topic_id),
    enabled: session.data?.embedding_model === true,
  });
  const navigate = useNavigate();
  const merge = useEdit((into: string) => api.mergeTopic(topic.topic_id, { into }));
  if (!similar.data || similar.data.length === 0) {
    return null;
  }
  return (
    <Section title="相似的 topic">
      <ul className="border-t border-line">
        {similar.data.map((s) => (
          <li key={s.topic.topic_id} className="flex items-center gap-3 border-b border-line py-2">
            <Link
              to="/topics/$topicId"
              params={{ topicId: s.topic.topic_id }}
              className="min-w-0 flex-1 truncate text-ink hover:text-brand-ink"
            >
              {s.topic.label}
            </Link>
            <span className="shrink-0 text-xs text-ink-faint tabular-nums">
              {Math.round(s.similarity * 100)}%
            </span>
            <AskCount count={s.ask_count} />
            <Button
              disabled={merge.isPending}
              onClick={() => {
                if (window.confirm(`把「${topic.label}」合并进「${s.topic.label}」？`)) {
                  merge.mutate(s.topic.topic_id, {
                    onSuccess: (merged) =>
                      navigate({ to: "/topics/$topicId", params: { topicId: merged.topic_id } }),
                  });
                }
              }}
            >
              <GitMerge className="size-4" aria-hidden />
              合并进去
            </Button>
          </li>
        ))}
      </ul>
      {merge.error && <ErrorBox error={merge.error} />}
    </Section>
  );
}
