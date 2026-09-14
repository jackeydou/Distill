# Distill 使用流程

[README](README.md) 的配套文件：按用户实际经历的顺序，把各项决定串起来。每一步后面标出它依据
的决定编号。

## 1. 安装和初始化（每台设备一次）

`distill` 不改 Codex 和 Claude Code 的配置，插件由用户自己装（D5）。两种方式：

- **把提示词发给 agent**：在 Codex 或 Claude Code 里输入"按 <url>/INSTALL.md 安装 Distill"。
  agent 依次完成下面四步，需要选择时问用户。
- **手动或在桌面 App 里**：按下面四步自己做；第 3 步也可以在 Codex App、Claude 桌面端的插件
  界面里添加 marketplace。

1. 安装 `distill` 二进制。现在是 `cargo install --path crates/distill-cli`，以后用
   Homebrew / winget / Scoop（D10）。
2. `distill init`：
   - 选 vault 目录。macOS 有 iCloud Drive 时推荐 `iCloud Drive/Distill`，否则默认文稿目录下的
     `Distill`；也可以填任意路径（D2）。
   - 如果目录里已经有 `distill.toml`，识别为已有 vault（第二台设备的情况），只建本机索引。
   - 生成设备 id，选定 Web UI 端口，记下 `distill` 自身路径，写入本机配置（D7、D10）。
3. 添加 Distill 的 marketplace 并安装插件（skill、MCP server、提问 hook）：
   `codex plugin marketplace add <owner/repo>`、`claude plugin marketplace add <owner/repo>`，
   再各自安装。
4. `distill doctor`：检查 vault、索引、插件版本和 hook 是否都生效。然后开一个新会话。

## 2. 日常提问（每次提问，用户无感）

1. 用户在 Codex 或 Claude Code 里提问。
2. 提问 hook 运行，几毫秒（D12）：注入 `distill-source`、`distill-suggest` 两行，并提醒 agent
   本会话还没读过 distill skill 就先读。不记录提问，不写任何文件。
3. 如果是概念、原理、方案类问题，agent 按 skill 先调 `distill_recall`。以前问过的话，把上次的
   结论和你的标注带进回答，并告诉你"第 N 次问，上次的 note 在这"。
4. agent 回答。如果这一轮值得以后回看，在末尾问一次"要不要 distill？"。同一会话里同一个
   问题只问一次，你拒绝后不再问。

## 3. Distill（用户同意建议，或主动发起）

1. 用户回答"要"，或者自己用插件发起（Claude Code 里 `/distill`，Codex 里调用 distill skill）。
2. agent 调 `distill_recall`，传入这次的核心问题。返回（D3、D5、D11）：
   - 相似的已有 note、它们的标注和所属 topic 的提问次数；
   - 全部已有 tag 及使用次数。
3. agent 判断（README 的 Open questions，原 Q14）：
   - 这是不是某个已有 topic 的同一个问题 → `topic` 填已有 id，否则填 `new`；
   - tag 优先用已有的，确实没有合适的才放进 `new_tags`（D5）。
4. agent 按 note 格式整理：标题、问题、结论、要点、仍不清楚的点。
5. agent 调 `distill_save`。服务端校验来源 session id 和 tag，写入 vault 里的 Markdown 文件，更新
   索引（D2、D5）。
6. 返回给 agent：note 链接 `http://distill.localhost:<port>/notes/<id>`，以及"这是第 N 次问"。
   需要时自动拉起后台 Web 服务，保证链接点得开（D7）。
7. agent 告诉用户：已保存、链接、第几次问。

## 4. 回顾（Web UI）

入口：点 agent 给的链接，或在终端执行 `distill ui`。首次在某个浏览器打开时通过 `distill ui`
授权，之后 cookie 长期有效（D7）。

| 页面 | 内容 |
|---|---|
| 首页 | 最近的 note、问得最多的 topic、还没标注的 note、需要处理的冲突 |
| topic 页 | 这个问题的所有 note，按时间排开，能看出理解怎么变化 |
| note 页 | 正文、tag、标注（可新增、编辑、删除）、"打开原会话"：深链加兜底命令（D6、D11） |
| tag 页 | 这个 tag 下的 note |
| 搜索 | 命令面板，关键词搜 note（D3） |
| 统计 | 内容 P3 前定（原 Q13） |

用户在页面上能改的：标注、处理冲突副本、合并 topic（P3 前定，原 Q11）。改 note 正文用自己的编辑器直接打开
文件，保存后索引自动发现变化（D2）。

## 5. 让 agent 参与分析

在任意 Codex / Claude Code 会话里直接提要求，比如"分析一下我这个月问得最多的 topic，哪些我
可能还没真正理解"。agent 调 `distill_stats`、`distill_search`、`distill_recall` 拿到 JSON，再做
分析。脚本或其他工具可以用 `distill stats --json` 等命令（D1、D5）。

## 6. 重问

用户再次问到以前问过的问题时，有三个时机能发现：

1. **提问时**：agent 按 skill 先查 Distill，把上次的 note 和用户标注带进回答。这是解决"找不到、
   只好重问"最直接的时机。v1 一定有（D12）。
2. **distill 时**：`distill_recall` 命中同一 topic，保存后告诉用户"第 N 次问"。v1 一定有。
3. **回顾时**：Web UI 的 topic 页和统计页显示重问次数。v1 一定有。

## 7. 多设备

- vault 由 iCloud / Dropbox / Syncthing 等工具同步；索引在每台设备本机，发现新文件后增量
  更新（D2）。
- note、标注都是新建文件，基本不冲突。万一出现冲突副本，首页提示处理（D2）。

## 8. 维护

| 命令 | 作用 |
|---|---|
| `distill doctor` | 检查 vault、索引、插件、hook；保存被拒时首先运行它 |
| `distill reindex` | 从 vault 全量重建索引 |
| `distill vault show / move / use` | 查看、迁移、切换 vault（D2） |
| `distill ui stop` | 停掉后台 Web 服务（D7） |
| `distill config set …` | `suggest.enabled`、`ui.port` 等开关 |
