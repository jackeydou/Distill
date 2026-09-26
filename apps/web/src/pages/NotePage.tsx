import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import { ExternalLink, FileText } from "lucide-react";
import type { NoteDetail } from "../api/generated/NoteDetail";
import { q } from "../api/queries";
import { Annotations } from "../components/Annotations";
import { Markdown } from "../components/Markdown";
import { AskCount, CopyButton, ErrorBox, Loading, Section, TagChip } from "../components/ui";
import { agentLabel, formatDateTime, projectName, stripTitle } from "../lib/format";

export function NotePage() {
  const { noteId } = useParams({ from: "/notes/$noteId" });
  const note = useQuery(q.note(noteId));
  if (note.isPending) {
    return <Loading />;
  }
  if (note.isError) {
    return <ErrorBox error={note.error} />;
  }
  const d = note.data;
  return (
    <article>
      <Link
        to="/topics/$topicId"
        params={{ topicId: d.note.topic_id }}
        className="group mb-4 inline-flex max-w-full items-center gap-2 text-sm text-ink-faint hover:text-brand-ink"
      >
        <span className="shrink-0">Topic</span>
        <span className="truncate underline-offset-4 group-hover:underline">{d.topic_label}</span>
        <AskCount count={d.ask_count} />
      </Link>
      <h1 className="text-[1.875rem] leading-tight sm:text-4xl">{d.note.title}</h1>
      <Meta detail={d} />
      {d.note.tags.length > 0 && (
        <div className="mt-4 flex flex-wrap gap-1.5">
          {d.note.tags.map((tag) => (
            <TagChip key={tag} tag={tag} />
          ))}
        </div>
      )}
      <div className="mt-8">
        <Markdown source={stripTitle(d.markdown)} />
      </div>
      <Section title="Annotation">
        <Annotations noteId={d.note.id} annotations={d.note.annotations} />
      </Section>
      <Section title="原会话">
        <Reopen detail={d} />
      </Section>
    </article>
  );
}

function Meta({ detail }: { detail: NoteDetail }) {
  const { note } = detail;
  return (
    <dl className="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-sm text-ink-faint">
      <div>
        <dt className="sr-only">时间</dt>
        <dd>
          <time dateTime={note.created}>{formatDateTime(note.created)}</time>
        </dd>
      </div>
      <div>
        <dt className="sr-only">来源</dt>
        <dd>{agentLabel[note.source.agent]}</dd>
      </div>
      <div className="min-w-0">
        <dt className="sr-only">项目</dt>
        <dd className="truncate" title={note.source.git_repo ?? note.source.cwd}>
          {projectName(note.source)}
        </dd>
      </div>
    </dl>
  );
}

function Reopen({ detail }: { detail: NoteDetail }) {
  const { reopen, note } = detail;
  return (
    <div className="space-y-3">
      <a
        href={reopen.link}
        className="inline-flex items-center gap-2 text-brand-ink underline underline-offset-4"
      >
        <ExternalLink className="size-4" aria-hidden />在 {agentLabel[note.source.agent]}{" "}
        里打开这次会话
      </a>
      <p className="text-sm text-ink-faint">
        链接没反应时（没装桌面 App，或 App 改了链接格式），在终端运行：
      </p>
      <div className="flex items-center gap-1 rounded-panel border border-line bg-card pl-3">
        <code className="min-w-0 flex-1 overflow-x-auto py-2 font-mono text-sm whitespace-nowrap text-ink">
          {reopen.command}
        </code>
        <CopyButton text={reopen.command} label="复制命令" />
      </div>
      <div className="flex items-center gap-2 text-sm text-ink-faint">
        <FileText className="size-4 shrink-0" aria-hidden />
        <span className="min-w-0 truncate font-mono text-xs" title={detail.file}>
          {detail.file}
        </span>
        <CopyButton text={detail.file} label="复制文件路径" />
      </div>
    </div>
  );
}
