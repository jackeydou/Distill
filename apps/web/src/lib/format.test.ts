import { describe, expect, it } from "vitest";
import { dayLabel, excerpt, formatAgo, projectName, stripTitle, withoutSection } from "./format";

describe("formatAgo", () => {
  const now = new Date("2026-09-20T12:00:00Z");
  it("picks the largest unit", () => {
    expect(formatAgo("2026-09-17T12:00:00Z", now)).toBe("3天前");
    expect(formatAgo("2026-09-20T11:59:30Z", now)).toBe("刚刚");
    expect(formatAgo("2026-09-06T12:00:00Z", now)).toBe("2周前");
  });
});

describe("projectName", () => {
  it("prefers the git remote", () => {
    expect(projectName({ git_repo: "https://github.com/me/app.git", cwd: "/x" })).toBe(
      "github.com/me/app",
    );
  });
  it("falls back to the tail of cwd", () => {
    expect(projectName({ cwd: "/Users/me/code/distill" })).toBe("code/distill");
    expect(projectName({ cwd: "C:\\work\\app" })).toBe("work/app");
  });
});

describe("stripTitle", () => {
  it("drops only the first H1", () => {
    expect(stripTitle("# Title\n\n## 问题\n\n# not a title")).toBe("\n## 问题\n\n# not a title");
  });
});

describe("excerpt", () => {
  it("keeps prose and drops code and markers", () => {
    expect(
      excerpt("不能直接用：`bun:sqlite` **很慢**。\n\n```ts\nx()\n```\n- [文档](http://a)"),
    ).toBe("不能直接用：bun:sqlite 很慢。 文档");
  });
});

describe("dayLabel", () => {
  const now = new Date("2026-09-26T10:00:00");
  it("names today and yesterday", () => {
    expect(dayLabel("2026-09-26", now)).toBe("今天");
    expect(dayLabel("2026-09-25", now)).toBe("昨天");
    expect(dayLabel("2026-09-13", now)).toContain("9月13日");
    expect(dayLabel("2025-12-31", now)).toContain("2025");
  });
});

describe("withoutSection", () => {
  it("drops one section and keeps the rest", () => {
    const md = "## 问题\n\nq\n\n## 结论\n\nc\n";
    expect(withoutSection(md, "问题")).toBe("## 结论\n\nc\n");
    expect(withoutSection(md, "结论")).toBe("## 问题\n\nq\n");
    expect(withoutSection(md, "要点")).toBe(md);
  });
});
