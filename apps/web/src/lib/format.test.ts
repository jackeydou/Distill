import { describe, expect, it } from "vitest";
import { excerpt, formatAgo, projectName, stripTitle } from "./format";

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
