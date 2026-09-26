import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import type { Conflict } from "../api/generated/Conflict";
import { q, useEdit } from "../api/queries";
import { Button, Empty, ErrorBox, Loading, PageHeader, Panel, Section } from "../components/ui";

export function ProblemsPage() {
  const problems = useQuery(q.problems());
  if (problems.isPending) {
    return <Loading />;
  }
  if (problems.isError) {
    return <ErrorBox error={problems.error} />;
  }
  const { conflicts, invalid_files } = problems.data;
  return (
    <div>
      <PageHeader title="需要处理" />
      <Section title="同步冲突">
        <p className="mb-4 text-sm text-ink-faint">
          两台设备同时改了同一个文件，同步工具留下了多份。选一份保留；其余的移到本机数据目录的
          <code className="mx-1 font-mono text-xs">discarded/</code>，不会删除。
        </p>
        {conflicts.length === 0 ? (
          <Empty>没有同步冲突。</Empty>
        ) : (
          <div className="space-y-4">
            {conflicts.map((c) => (
              <ConflictCard key={`${c.kind}:${c.id}`} conflict={c} />
            ))}
          </div>
        )}
      </Section>
      <Section title="读不了的文件">
        <p className="mb-4 text-sm text-ink-faint">
          这些文件格式不对，没有进入索引。用编辑器修好后会自动重新读取。
        </p>
        {invalid_files.length === 0 ? (
          <Empty>所有文件都读得了。</Empty>
        ) : (
          <ul className="space-y-2">
            {invalid_files.map((f) => (
              <li key={f.path} className="rounded-panel border border-line bg-panel px-3 py-2">
                <div className="font-mono text-sm text-ink">{f.path}</div>
                <div className="mt-1 text-sm text-error">{f.error}</div>
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}

function ConflictCard({ conflict }: { conflict: Conflict }) {
  const resolve = useEdit((keep: string) =>
    api.resolveConflict({ kind: conflict.kind, id: conflict.id, keep }),
  );
  return (
    <Panel className="p-4">
      <div className="text-sm text-ink-faint">
        {conflict.kind} <span className="font-mono text-xs">{conflict.id}</span>
      </div>
      <ul className="mt-3 space-y-2">
        {conflict.paths.map((path) => (
          <li key={path} className="flex items-center gap-3">
            <span className="min-w-0 flex-1 truncate font-mono text-sm text-ink" title={path}>
              {path}
            </span>
            <Button disabled={resolve.isPending} onClick={() => resolve.mutate(path)}>
              保留这份
            </Button>
          </li>
        ))}
      </ul>
      {resolve.error && (
        <div className="mt-3">
          <ErrorBox error={resolve.error} />
        </div>
      )}
    </Panel>
  );
}
