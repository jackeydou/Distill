# Writing a note

A note is read months later, often by the same user asking the same question again. Write it
so it stands on its own without the conversation.

| Field | Write |
|---|---|
| `title` | One line naming the question, e.g. "bun:sqlite 为什么加载不了扩展" |
| `question` | The underlying question, not the literal first prompt. One or two sentences |
| `conclusion` | The answer the conversation reached, in a few sentences. Say why, not only what |
| `key_points` | Up to five short points worth remembering: a command, a rule, a gotcha |
| `open_questions` | What is still unclear or was left unanswered. Empty if nothing |

- Use the user's language for every field.
- Short code or command snippets are fine; do not paste long files or logs.
- Do not include secrets. Distill redacts known formats, but do not rely on it.
- If the answer changed during the conversation, record the final one.

## Example `distill_save` call

```json
{
  "title": "bun:sqlite 为什么加载不了扩展",
  "question": "在 macOS 上用 bun:sqlite 加载 sqlite-vec 报错，原因是什么，怎么解决？",
  "conclusion": "bun:sqlite 在 macOS 上用系统自带的 SQLite，它编译时关闭了动态扩展加载，所以 loadExtension 必然失败。要么用 Database.setCustomSQLite 指向自带的 libsqlite3，要么换成把 SQLite 静态编译进去的方案（如 rusqlite bundled）。",
  "key_points": [
    "报错信息：This build of sqlite3 does not support dynamic extension loading",
    "node:sqlite 自带 SQLite，可以加载扩展"
  ],
  "open_questions": ["Linux 上的 bun 是否同样受限"],
  "topic": "new",
  "tags": ["sqlite"],
  "new_tags": ["bun"],
  "source": {
    "agent": "codex",
    "session_id": "01a0d530-42ae-7731-8a1d-b4e07d9b837d",
    "cwd": "/Users/me/project"
  }
}
```
