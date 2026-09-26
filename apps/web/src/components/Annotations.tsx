import { Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../api/client";
import type { AnnotationView } from "../api/generated/AnnotationView";
import { useEdit } from "../api/queries";
import { formatDateTime } from "../lib/format";
import { Markdown } from "./Markdown";
import { Button, ErrorBox } from "./ui";

/** The user's own understanding of a note, one entry per annotation file. */
export function Annotations({
  noteId,
  annotations,
}: {
  noteId: string;
  annotations: AnnotationView[];
}) {
  const add = useEdit((text: string) => api.addAnnotation(noteId, { text }));
  return (
    <div className="space-y-4">
      {annotations.length === 0 && (
        <p className="text-sm text-ink-faint">
          还没有 annotation。用自己的话写下你现在的理解，下次重问时 agent 会先看到它。
        </p>
      )}
      {annotations.map((a) => (
        <AnnotationItem key={a.id} annotation={a} />
      ))}
      <Editor
        key={annotations.length}
        submitLabel="添加"
        pending={add.isPending}
        error={add.error}
        onSubmit={(text) => add.mutate(text)}
      />
    </div>
  );
}

function AnnotationItem({ annotation }: { annotation: AnnotationView }) {
  const [editing, setEditing] = useState(false);
  const update = useEdit((text: string) => api.updateAnnotation(annotation.id, { text }));
  const remove = useEdit(() => api.deleteAnnotation(annotation.id));

  if (editing) {
    return (
      <Editor
        initial={annotation.body}
        submitLabel="保存"
        pending={update.isPending}
        error={update.error}
        onCancel={() => setEditing(false)}
        onSubmit={(text) => update.mutate(text, { onSuccess: () => setEditing(false) })}
      />
    );
  }
  return (
    <div className="rounded-panel border-l-2 border-brand bg-panel py-3 pr-2 pl-4">
      <Markdown source={annotation.body} />
      <div className="mt-2 flex items-center gap-1 text-xs text-ink-faint">
        <span>
          {formatDateTime(annotation.created)}
          {annotation.updated && ` · 改于 ${formatDateTime(annotation.updated)}`}
        </span>
        <span className="ml-auto" />
        <IconButton label="编辑" onClick={() => setEditing(true)}>
          <Pencil className="size-3.5" />
        </IconButton>
        <IconButton
          label="删除"
          disabled={remove.isPending}
          onClick={() => {
            if (window.confirm("删除这条 annotation？文件会从 vault 里移除。")) {
              remove.mutate(undefined);
            }
          }}
        >
          <Trash2 className="size-3.5" />
        </IconButton>
      </div>
      {remove.error && <ErrorBox error={remove.error} />}
    </div>
  );
}

function Editor({
  initial = "",
  submitLabel,
  pending,
  error,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  submitLabel: string;
  pending: boolean;
  error: Error | null;
  onSubmit: (text: string) => void;
  onCancel?: () => void;
}) {
  const [text, setText] = useState(initial);
  const submit = () => {
    if (text.trim()) {
      onSubmit(text);
    }
  };
  return (
    <form
      className="space-y-2"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            submit();
          }
        }}
        rows={4}
        placeholder="我的理解…（支持 Markdown，⌘Enter 保存）"
        aria-label="Annotation"
        className="w-full resize-y rounded-panel border border-line-strong bg-panel px-3 py-2 text-[0.9375rem] text-ink placeholder:text-ink-faint focus:border-brand focus:outline-none"
      />
      {error && <ErrorBox error={error} />}
      <div className="flex gap-2">
        <Button type="submit" variant="primary" disabled={pending || !text.trim()}>
          {submitLabel}
        </Button>
        {onCancel && <Button onClick={onCancel}>取消</Button>}
      </div>
    </form>
  );
}

function IconButton({
  label,
  children,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="inline-flex size-7 items-center justify-center rounded-[1px] transition-colors duration-150 hover:bg-panel-hover hover:text-ink disabled:opacity-50"
      {...props}
    >
      {children}
    </button>
  );
}
