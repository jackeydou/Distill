import { useQuery } from "@tanstack/react-query";
import { Command } from "cmdk";
import type { TopicCount } from "../api/generated/TopicCount";
import { q } from "../api/queries";
import { Button, ErrorBox, Loading } from "./ui";

/** Choose a topic by typing part of its label. */
export function TopicPicker({
  exclude,
  onPick,
  onCancel,
}: {
  exclude: string;
  onPick: (topic: TopicCount) => void;
  onCancel: () => void;
}) {
  const topics = useQuery(q.topics());
  if (topics.isPending) {
    return <Loading />;
  }
  if (topics.isError) {
    return <ErrorBox error={topics.error} />;
  }
  const candidates = topics.data.filter((t) => t.topic_id !== exclude);
  return (
    <div className="rounded-panel border border-line-strong bg-panel">
      <Command label="选择要合并进的 topic">
        <Command.Input
          autoFocus
          placeholder="输入 topic 名称…"
          className="h-10 w-full border-b border-line bg-transparent px-3 text-sm text-ink outline-none placeholder:text-ink-faint"
        />
        <Command.List className="max-h-72 overflow-y-auto p-1">
          <Command.Empty className="px-3 py-4 text-sm text-ink-faint">没有其他 topic</Command.Empty>
          {candidates.map((t) => (
            <Command.Item
              key={t.topic_id}
              value={`${t.label} ${t.topic_id}`}
              onSelect={() => onPick(t)}
              className="flex cursor-pointer items-center gap-2 rounded-[1px] px-2 py-2 text-sm data-[selected=true]:bg-brand-soft data-[selected=true]:text-brand-ink"
            >
              <span className="truncate">{t.label}</span>
              <span className="ml-auto shrink-0 text-xs text-ink-faint">{t.ask_count} 次</span>
            </Command.Item>
          ))}
        </Command.List>
      </Command>
      <div className="border-t border-line p-2">
        <Button onClick={onCancel}>取消</Button>
      </div>
    </div>
  );
}
