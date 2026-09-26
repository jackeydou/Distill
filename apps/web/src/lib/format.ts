import type { Agent } from "../api/generated/Agent";
import type { Source } from "../api/generated/Source";

const dateFormat = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});
const dateTimeFormat = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});
const relative = new Intl.RelativeTimeFormat("zh-CN", { numeric: "auto" });

/** `2026-09-13` from an RFC 3339 timestamp, in the viewer's time zone. */
export function formatDate(rfc3339: string): string {
  return dateFormat.format(new Date(rfc3339));
}

export function formatDateTime(rfc3339: string): string {
  return dateTimeFormat.format(new Date(rfc3339));
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

/** "3 天前"; under a minute is "刚刚". */
export function formatAgo(rfc3339: string, now: Date = new Date()): string {
  const seconds = (new Date(rfc3339).getTime() - now.getTime()) / 1000;
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) {
      return relative.format(Math.round(seconds / size), unit);
    }
  }
  return "刚刚";
}

export const agentLabel: Record<Agent, string> = {
  codex: "Codex",
  "claude-code": "Claude Code",
};

/** The project a note came from: the git remote when known, else the last path segments. */
export function projectName(source: Pick<Source, "git_repo" | "cwd">): string {
  if (source.git_repo) {
    return source.git_repo.replace(/^https?:\/\//, "").replace(/\.git$/, "");
  }
  const parts = source.cwd.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("/") || source.cwd;
}

/** The body of a note without its leading `# title` line, which the page renders itself. */
export function stripTitle(markdown: string): string {
  return markdown.replace(/^\s*# [^\n]*\n?/, "");
}

/** Plain text for a one- or two-line preview: code blocks and Markdown markers removed. */
export function excerpt(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?(```|$)/g, " ")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+\.)\s+/gm, "")
    .replace(/[`*_~]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
