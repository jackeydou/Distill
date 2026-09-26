import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

/** Note and annotation Markdown. Raw HTML in the source is shown as text, never rendered. */
export function Markdown({ source }: { source: string }) {
  return (
    <div className="prose-note">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{source}</ReactMarkdown>
    </div>
  );
}
