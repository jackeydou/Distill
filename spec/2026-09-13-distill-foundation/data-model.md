# Distill 数据模型（草案）

[README](README.md) 的配套文件。术语见 README 的 [Terms](README.md#terms)。

## vault（真实来源，会同步）

```
Distill/
  notes/2026/09/01J8Z3K9Q2-bun-sqlite-extension-loading.md
  topics/01J8Z3K7AB.md        # label、简介、merged_into?
  tag-aliases/01J8Z4....yml   # { from, to }
  annotations/<note-id>/01J8Z5....md   # 用户标注，一条一个文件（D11）
  distill.toml                # vault 格式版本等
```

note 文件：

```markdown
---
schema: 1
id: 01J8Z3K9Q2...
topic: 01J8Z3K7AB...
tags: [sqlite, bun]
source:                        # agent、session_id 必填（D5）
  agent: codex                 # codex | claude-code
  session_id: 01a0d530-42ae-7731-8a1d-b4e07d9b837d
  cwd: /Users/me/project
  git_repo: github.com/me/project   # 可选
created: 2026-09-13T14:03:00+08:00
---

# bun:sqlite 在 macOS 上加载扩展

## 问题
## 结论
## 要点
## 仍不清楚
```

文件格式本身就是对外契约，改动要升 `schema` 版本，并在读取时兼容旧版本。定稿后格式说明移到
`docs/`。

## 索引（本机，可重建）

```
file         path, mtime, size, hash, kind(note|topic|alias|prompts), error?
note         id, topic_id, title, question, conclusion, created_at, source_*, path
topic        id, label, merged_into?        -- 查询时解析到最终 topic
tag / note_tag
note_fts     FTS5
embedding    vec0 虚表                          -- P4
annotation   id, note_id, body, anchor?, created_at, updated_at, path
conflict     id, path                           -- 同一 frontmatter id 对应多个文件
```

提问次数 = 某个 topic（按 `merged_into` 解析后）下的 note 数，按跨越的 session 数、时间跨度
分组。同一 topic 下的多条 note 按时间排开，能看出理解是怎么
变化的。
