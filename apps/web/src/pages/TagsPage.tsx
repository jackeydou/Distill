import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import { q } from "../api/queries";
import { BarList } from "../components/charts";
import { NoteList } from "../components/NoteList";
import { Empty, ErrorBox, Loading, PageHeader } from "../components/ui";

export function TagsPage() {
  const tags = useQuery(q.tags());
  if (tags.isPending) {
    return <Loading />;
  }
  if (tags.isError) {
    return <ErrorBox error={tags.error} />;
  }
  return (
    <div>
      <PageHeader title="Tags">{tags.data.length} 个 tag，按 note 数排列。</PageHeader>
      {tags.data.length === 0 ? (
        <Empty>还没有 tag。</Empty>
      ) : (
        <BarList
          bars={tags.data.map((t) => ({ key: t.tag, label: `#${t.tag}`, value: t.notes }))}
          link={(b) => ({ to: "/tags/$tag", params: { tag: b.key } })}
        />
      )}
    </div>
  );
}

export function TagPage() {
  const { tag } = useParams({ from: "/tags/$tag" });
  const notes = useQuery(q.notes({ tag, limit: 500 }));
  return (
    <div>
      <PageHeader eyebrow={<Link to="/tags">Tags</Link>} title={`#${tag}`}>
        {notes.data && `${notes.data.length} 条 note`}
      </PageHeader>
      {notes.isPending ? (
        <Loading />
      ) : notes.isError ? (
        <ErrorBox error={notes.error} />
      ) : (
        <NoteList notes={notes.data} empty="这个 tag 下没有 note。" />
      )}
    </div>
  );
}
