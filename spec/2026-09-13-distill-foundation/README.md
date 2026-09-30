# Distill：产品形态与基础架构

## Status

`accepted`（2026-09-13）。此后的修改以带日期的备注写在对应小节下。配套文件：使用流程
[flows.md](flows.md)，数据模型 [data-model.md](data-model.md)。

## Terms

术语统一用英文，文档、代码、界面里都不翻译（2026-09-13 讨论确定）。

| Term | 含义 |
|---|---|
| note | 一次 distill 的产物：一个 Markdown 文件，包含问题、结论、要点、仍不清楚的点 |
| topic | 同一个问题。问的是同一件事的 note 归到同一个 topic；每条 note 恰好属于一个 topic。重问次数 = topic 下的 note 数 |
| tag | 分类标签，粒度比 topic 粗；每条 note 1–5 个，一个 tag 下有很多不同的 topic |
| annotation | 用户在 Web UI 上给 note 加的标注，记录自己的理解 |
| vault | 存放 note、topic、annotation 等 Markdown 文件的目录，是唯一的真实数据来源，可由同步工具同步 |
| index | 从 vault 算出来的本机 SQLite 索引，可随时重建 |
| source | note 的来源：agent（`codex` / `claude-code`）、session id、cwd |
| distill | 动作：把当前会话里的一次问答整理成 note 存进 vault |
| recall | 动作：给一个问题，查找以前是否问过、有哪些相关 note |

## Request

1. 每天大量时间在和 AI 对话，但 session 太多，回头找不到，于是倾向于重问一遍。重问说明两件事：
   当时没真正理解；以及重问的次数反映了我对这个问题的关心和投入。
2. 需要一个地方聚合整理和 agent 的对话：按内容打 tag 分类，做统计，统计结果能作为 context
   交给 agent 参与分析。
3. 给 Codex 和 Claude Code 提供 plugin，让用户在会话里主动发起 distill，把结果存到 Distill。
   存储考虑本地 PGlite 或 SQLite；评估是否需要 vector DB 检索历史 distilled notes。
4. 产品形态倾向 CLI + App（App 通过 CLI 取数据，类似 Codex desktop app）。从可维护性和
   可扩展性的角度审视这个选型。App 技术栈参考
   [reflect-open](https://github.com/team-reflect/reflect-open)。

讨论后改为：v1 只做 `distill` CLI，界面是打包在 CLI 里的本机 Web，不做桌面端（见 D7）。
前端是标准 React CSR 单页应用（见 D9）。

## 先把问题说清楚

**不回填历史。** Distill 从安装那一刻开始记录，不导入、不扫描之前的会话。第一次 distill
建立历史，下一次重问时就有东西可以匹配。（2026-09-13 讨论确定。）

**重问怎么被发现。** 每次 `distill_save` 时，core 先用新 note 的问题去匹配已有 note。
命中就把两者归到同一个 topic，topic 的提问次数加一，并告诉用户"这是你第 N 次问这个，
上次的 note 在这"。开新会话提问时，agent 也可以先调 `distill_recall` 查一次。

只统计 distill 过的问题：问了但没有 distill 的不计数，也不记录提问原文（2026-09-13 讨论确定，
理由见 Rejected）。这个盲区靠两点缩小：值得记的问题 agent 会主动建议 distill（D12）；提问时
agent 可以先查 Distill。

| 层 | 内容 | 来源 | 是否需要 LLM |
|---|---|---|---|
| L0 原始会话 | agent 自己的 JSONL | 留在原地，Distill 只存路径 | 否 |
| L2 distilled note | 结构化笔记：问题、结论、要点、仍不清楚的点、tag | 用户在会话里主动触发，由宿主 agent 生成 | 是（宿主 agent 的） |
| L3 派生数据 | topic、提问次数、tag 归类、统计 | 从 L2 计算 | 第一版由宿主 agent 判断，见 D3 |

L2 是唯一不可重建的数据，以 Markdown 文件保存，随文件夹同步和备份；其余都能从文件重建（见 D2）。

## Decisions

### D1. Core 先行：一个 core，三个入口

```
                ┌──────────────────────── crates/distill-core ─────────────────────┐
                │  sources/ (codex, claude-code 适配)     notes/   tags/   stats/    │
                │  vault/ (Markdown 读写)   search/ (FTS + 向量)   db/ (SQLite 索引) │
                └──────────────────────────────────────────────────────────────────┘
                       ▲                      ▲                        ▲
          distill <cmd> --json      distill mcp (stdio)      distill ui (HTTP, 回环地址)
                       ▲                      ▲                        ▲
                 人 / 脚本 / 宿主 agent   Codex / Claude Code 插件      浏览器里的 Web UI
```

所有业务逻辑在 `crates/distill-core`（语言见 D10）。`distill` 是**一个**可执行文件，有三种
入口：普通子命令、MCP server、`ui`（本机 HTTP 服务，同时提供 Web 静态资源和 API）。Web UI 不直接碰数据库，
只通过这组 API 取数据。

为什么这样切：

- **协议是产品的稳定契约，界面不是。** CLI、MCP、Web UI，以及以后可能的桌面壳、VS Code
  扩展、Raycast，都消费同一组操作。换界面不影响数据和逻辑。
- **类型只定义一次。** reflect-open 的 DB 在 Rust、业务在 TS，他们的 backlog 里记着：每个
  写操作的 payload 要在 zod 和 serde 里各写一遍、手工同步（`db/write.rs:14`、
  `0002-index-bridge-followups.md` #3）。这里 API 类型只在 Rust 里定义，TS 类型由 `ts-rs`
  生成，CI 检查生成结果没有漂移。
- Codex desktop 也是这个结构：UI 进程拉起 `codex app-server`，走 JSON-RPC。

对"App 通过 CLI 取数据"的修正：**不要每次请求 spawn 一个 CLI 再解析 stdout。** 启动开销、
错误处理、流式和订阅都会变得很难看。`distill ui` 是一个长驻进程，Web UI 通过带 schema 的
RPC 调它（请求和响应类型从 Rust 生成），数据变化用 SSE 推给页面。CLI 子命令和 RPC 方法共用同一批 core 函数，所以本质上仍是"界面通过 CLI"。

### D2. 存储：Markdown 文件为准，SQLite 只做本机索引

note 等不可重建的数据写成 Markdown 文件，放在一个 **vault** 文件夹里；SQLite 是从 vault
算出来的本机索引，删掉可以用 `distill reindex` 重建（2026-09-13 讨论确定）。和 reflect-open
同构，目的是：

- **同步交给文件夹。** Distill 不调用任何同步服务的 API，vault 放进哪个同步目录就由哪个工具
  同步。macOS 上推荐 iCloud Drive；Windows 和 Linux 不做 iCloud 相关的支持（2026-09-13 讨论
  确定），用 Dropbox、OneDrive、Syncthing 这类工具：

  | 工具 | macOS | Windows | Linux |
  |---|---|---|---|
  | iCloud Drive | 推荐，有 `icloud:` 简写 | 不支持 | 无客户端 |
  | Dropbox | 可用 | 可用 | 官方客户端 |
  | OneDrive | 可用 | 系统自带 | 只有第三方客户端 |
  | Syncthing | 可用 | 可用 | 可用；点对点，不经过云 |
  | git 仓库 | 可用 | 可用 | 可用；要自己提交和拉取 |

  一个 vault 只能由一种工具同步。设备里同时有 Mac 和 Windows / Linux 时，所有设备要用同一种
  工具，比如都用 Dropbox。
- **数据活得比 Distill 久。** 任何编辑器都能打开，agent 能直接 grep。
- **正在使用的 SQLite 不能放进 iCloud**，会损坏。所以索引只在本机。

#### 位置

| 内容 | 位置 | 是否同步 |
|---|---|---|
| vault | 用户选择，见下文；记录在本机配置里 | 取决于所选目录 |
| 索引 | 系统数据目录下 `index/<vault-id>.db` | 否 |
| 本机配置、设备 id | 系统配置目录下 `config.toml` | 否 |

系统目录用 `directories` crate 按平台取：macOS 是 `~/Library/Application Support/Distill/`，
Linux 是 `$XDG_DATA_HOME/distill/` 和 `$XDG_CONFIG_HOME/distill/`，Windows 是
`%APPDATA%\Distill\`。

#### 由用户选择 vault 目录

vault 放在哪里由用户决定，可以是 iCloud Drive 里的任意子目录，比如
`iCloud Drive/Notes/Distill`，也可以是任何本地或其他同步盘里的目录。

- **首次运行** `distill init`：macOS 上检测到 iCloud Drive 时，列出两个选项：
  `iCloud Drive/Distill`（推荐）和"自定义路径"。其他情况默认"文稿"目录下的 `Distill`
  （2026-09-13 讨论确定），并提示"要多设备同步，把 vault 放进 Dropbox、Syncthing 等同步目录"。
  不去探测各家同步盘的位置。非交互时用 `distill init --vault <path>`。

  "文稿"目录用 `directories` crate 的 `document_dir()` 取，不写死 `~/Documents`：Linux 上它
  读 `XDG_DOCUMENTS_DIR`，Windows 上开了 OneDrive 文件夹备份时，文稿目录会被重定向到
  `OneDrive\Documents`，这个函数能拿到真实位置。这两种情况下 vault 会被 iCloud（macOS 开了
  "桌面与文稿文件夹"同步时）或 OneDrive 自动同步，这没有问题，因为索引不在 vault 里。
- **路径写法**：CLI 接受任何真实路径。macOS 上另外接受 `icloud:Notes/Distill` 这种简写，展开为
  `~/Library/Mobile Documents/com~apple~CloudDocs/Notes/Distill`，显示时写成
  `iCloud Drive/Notes/Distill`。其他平台上用 `icloud:` 直接报错。
- **第二台设备**：`distill init --vault <path>` 指向已经同步过来的目录。vault 根目录的
  `distill.toml` 里有 vault id，Distill 据此识别出这是已有的 vault，只建本机索引，不重新初始化。
- **之后更换**：`distill vault move <path>` 复制到新位置，逐个文件校验后切换，原目录保留不删，
  作为恢复副本（reflect-open 的做法）。`distill vault use <path>` 切换到另一个已有 vault。
  `distill vault show` 显示当前 vault 和索引位置。
- **校验**：目录必须可写；vault 和索引目录不能互相包含；目录非空且没有 `distill.toml` 时拒绝，
  除非加 `--adopt`，避免把 Distill 文件撒进别的文件夹。
- **环境变量** `DISTILL_VAULT` 优先于本机配置，用于测试和临时切换。

用普通文件夹，而不是 iCloud 里带 App 图标的专属目录（ubiquity container）：专属目录需要带
iCloud entitlement 的签名 App，CLI 做不到，也没有必要。

Web UI 的设置页也能改 vault，但只能输入路径或用服务端提供的目录浏览：浏览器的文件夹选择器
拿不到绝对路径。这是 Web 相对桌面端的一个小劣势，以后做 Tauri 壳时换成原生选择框。

索引放在 vault **外面**。reflect-open 把 `.reflect/` 放在 vault 里再用 xattr 标记不同步，结果
碰上 iCloud 反复扫描排除目录、`fileproviderd` 长期占用 CPU 的问题（`docs/icloud-sync.md`）。
放外面就不会遇到。

#### 不冲突的写法

reflect-open 的冲突处理有约 5,000 行，是为"两台设备同时改同一条笔记"准备的。Distill 用写法
规则绕开这件事：**每台设备只新建文件，或者只追加写自己的文件。**

- 每条 note 一个文件，文件名带 ULID，两台设备不可能生成同一个文件名；
- note 生成后基本不改。要改由用户主动发起，概率低；
- topic 合并不重写所有 note，只在被合并的 topic 文件里写 `merged_into`，索引顺着链解析；
- tag 改名写一个新的别名文件，不去改每条 note；
- 用户标注不写进 note 文件，每条标注单独一个文件，见 D11。

仍可能冲突的只剩"两台设备同时改同一个文件"。各家同步工具都会留下一份冲突副本，但命名各不
相同：iCloud 是 `xxx 2.md`，Dropbox 是 `xxx (… conflicted copy 2026-09-13).md`，OneDrive 在
文件名后加设备名，Syncthing 是 `xxx.sync-conflict-20260913-…md`。所以 Distill **不按文件名
识别冲突**，而是看 frontmatter 里的 `id`：两个文件的 `id` 相同就是冲突副本，和用哪个工具
无关。Distill 不做自动合并，在 Web UI 标成"需要处理"，让用户选保留哪份。

索引只读 Distill 自己的目录和文件类型，忽略点开头的文件和目录（`.dropbox`、`.stfolder`、
`.stversions` 等），也忽略各家同步工具的临时文件。

#### 文件写入与索引更新

- 写文件用"临时文件 + rename"，保证原子性；
- 通过 core 写入时，同一进程里写完文件立刻更新索引；
- 外部变化（别的设备同步过来、用户手改）靠三种方式发现：`distill ui` 运行时的文件监听；
  每个命令执行前做一次增量扫描（按 mtime 和 size 比较，内容 hash 相同则跳过）；
  `distill reindex` 全量重建；
- 文件格式在读取时校验（这是外部输入的边界）。不合法的文件记录下来、跳过，不影响其他文件。

#### 索引数据库

SQLite + WAL，理由是**多进程并发**。同时会有这些进程读写索引：

- 每个开着的 Codex / Claude Code 会话各 spawn 一个 `distill mcp` 进程；
- 手动执行的 CLI；
- `distill ui` 的 HTTP 服务。

PGlite 是单连接的 WASM Postgres，一个数据目录同一时间只能被一个进程打开，多进程就得加一个
常驻 daemon。SQLite 在 WAL 模式下一写多读、跨进程，设好 `busy_timeout` 就够了。

- 表结构：`crates/distill-core/migrations/` 下的纯 SQL，`rusqlite_migration` 按
  `user_version` 执行（reflect-open 同款）。索引可重建，所以升级时如果迁移复杂，
  可以直接丢弃重建；
- 驱动：`rusqlite`（`bundled`），SQLite、FTS5、`sqlite-vec` 都静态链接，见 D10。

#### 需要 P1 验证

- **云盘腾空的文件。** macOS 开了"优化 Mac 储存空间"、Windows 上 iCloud 或 OneDrive 开了
  按需下载后，系统会把不常用的文件换成占位符。需要在两个平台上确认读取这类文件时是会触发
  下载、阻塞，还是直接失败。
- **同步延迟。** 刚在另一台设备保存的 note，可能要过一会儿才出现。这是正常现象，不需要处理。

### D3. 检索：FTS5 先行，向量放进同一个 SQLite，不单独上 vector DB

规模估算：个人数据一年大约几千条 note、几万条提问。5 万条 × 384 维 float32 ≈ 77 MB，
`sqlite-vec` 暴力扫描在几十毫秒级。Chroma / LanceDB / Qdrant 这类独立向量库带来的是第二份
存储、第二套备份和一致性问题，这个量级换不到任何收益。

但**向量不是可有可无的**：用户会换一种说法重问同一个问题，"重复提问"天然是语义匹配，
关键词匹配抓不到。所以 embedding 服务于两件事：

1. 保存 note 时匹配同一 topic → 提问次数（需求 1 的核心指标）；
2. `distill recall`：新问题进来时，找出以前问过的相似问题和已有的 note。

第一版可以不上 embedding：`distill_save` 和 `distill_recall` 用 FTS 取出 top-k 候选，由宿主
agent 判断是不是同一个问题。它本来就是 LLM，判断语义等价比阈值可靠。等出现"明明问过却没
匹配上"的情况，再加 embedding 提高候选召回。

> **2026-09-13 实现备注（P1）。** recall 没有走 trigram FTS：换了说法的问题很少与旧问题有三个
> 连续相同的字（"加载扩展" 对 "加载不了扩展"），冒烟测试里直接漏召回。改为在内存里对全部 note
> 按中文二字组和英文单词打分，按 IDF 加权，标题和问题里的命中权重加倍，覆盖不到问题总权重
> 25% 的丢弃。关键词搜索仍用 trigram FTS。实现：`crates/distill-core/src/index/recall.rs`。

> **2026-09-26 实现备注（P4）。** embedding 用 fastembed（ONNX Runtime 静态链接进 `distill`）加
> 量化版 `paraphrase-multilingual-MiniLM-L12-v2`（384 维，磁盘约 0.25 GB），这个模型专门训练过
> 同义句匹配，中英文都能用（2026-09-26 讨论确定，放弃了 candle + multilingual-e5-small 和调用本机
> Ollama）。模型不自动下载，`distill model pull` 显式拉取；没拉取时 recall 保持关键词打分。
> 向量按原计划放 `sqlite-vec` 的 `vec0` 表、用 KNN 查询。注册扩展要一段 `unsafe`，工作区的
> `unsafe_code` 从 `forbid` 改为 `deny`，只有 `index/vec.rs` 例外。note 向量按「模型 + 文本 hash」
> 做主键，`reindex` 不清它；topic 向量是 note 向量的均值，另存一张 `vec0` 表。融合用 RRF（k = 60）。阈值是实测出来的：12 对中英文问题里，
> 同一问题换说法得 0.40–0.74，不同问题最高 0.54，两段重叠，所以 recall 的下限定在 0.40、交给 agent
> 判断，合并建议的下限定在 0.65、交给用户确认。现状见 `docs/recall.md`。

照搬 reflect-open 的 `retrieve()`：一个函数提供 keyword / semantic / hybrid 三种模式，hybrid
用 RRF 融合，低于相似度阈值的丢弃。embedding 不可用时自动退回 FTS。

中文分词：FTS5 默认的 `unicode61` 不切中文，用 `trigram` 分词器。本机实测（2026-09-13，
`node:sqlite` 和 `rusqlite` 结果一致）：3 个字以上的中文查询 `MATCH` 能命中；2 个字的查询
`MATCH` 查不到，但同一张表上 `LIKE '%扩展%'` 能命中。所以查询少于 3 个字时退回 `LIKE`。
LIKE 是全表扫描，几千条 note 的规模没有问题。是否需要带 jieba 的分词扩展，等真实数据暴露问题再说。

### D4. 谁来做 distill：宿主 agent，不是 Distill

用户在 Codex 里发起 distill 时，完整上下文就在那个 agent 手里。让它按 schema 生成 note，
再调用 `distill_save`。好处：

- Distill 不需要 API key，也不需要自己的 LLM 调用和计费；
- 生成质量等于用户当下用的模型，上下文完整，不用重新读 JSONL。

后台任务（topic 命名、tag 整理）没有宿主 agent，见 Plan 的"推迟到后续阶段"。

### D5. 插件：一份源码，两个 manifest

Codex 插件和 Claude Code 插件的结构已经很接近：都是 manifest + `skills/` + MCP server 配置
（本机的 Codex 插件用 `.codex-plugin/plugin.json`，里面 `"skills": "./skills/"`、
`"mcpServers": "./.mcp.json"`）。所以：

```
plugins/distill/
  .codex-plugin/plugin.json
  .claude-plugin/plugin.json
  .mcp.json                 # command: distill mcp
  hooks/hooks.json          # UserPromptSubmit，见 D12
  skills/distill/SKILL.md   # agent 的全部行为规则，见 D12
```

> **2026-09-19 实现备注（P2）。** MCP 配置没有两边共用一份 `.mcp.json`：Claude Code 会自动加载
> 插件根目录的 `.mcp.json`，而 Codex 的写法是 `cwd: "."` 加相对路径，两边写法不兼容。所以 Claude
> Code 在 `plugin.json` 里内联声明，Codex 用 `codex.mcp.json`。hook 共用 `hooks/hooks.json`：
> Codex 同样展开 `${CLAUDE_PLUGIN_ROOT}`（见 Codex 二进制内的变量表）。Claude Code 里 skill 的
> 调用名是 `/distill:distill`。现状见 `docs/plugin.md`。
>
> **2026-09-25 实现备注。** 安装改为从 marketplace 分支：CI 在每次 push 到 main 时分别构建
> Codex 和 Claude Code 版本，提交到 `marketplace-codex`、`marketplace-claude` 两个分支，分支里只有
> 对应 agent 需要的文件，参照 jackeydou/codex-lang-coach。仓库根目录不再是 marketplace。因为每份
> 构建只服务一个 agent，两边都改回原生文件名（`.mcp.json`、`hooks/hooks.json`），各用各的变量：
> Claude Code 用 `${CLAUDE_PLUGIN_ROOT}`，Codex 用 `${PLUGIN_ROOT}`（Codex 也认前者，但只是兼容
> 别名）。上面 2026-09-19 那条里的 `codex.mcp.json` 和共用 hook 已不再适用。发布的版本号带构建
> 后缀 `+<agent>.<commit>`，agent 按版本缓存插件，不带后缀就看不到更新；源码里的版本号不变。
> 现状见 `docs/plugin.md`。

**安装方式：用户自己装，`distill` 不改 agent 的配置**（2026-09-13 讨论确定）。仓库同时提供
Codex 和 Claude Code 的插件 marketplace，两边都支持从 Git 仓库添加：

- Codex：`codex plugin marketplace add <owner/repo>`，再 `codex plugin add`；或在 Codex App 里添加。
- Claude Code：`claude plugin marketplace add <owner/repo>`，再 `claude plugin install`；或在
  Claude 桌面端里添加。

另外提供一段安装提示词，写在仓库的 `INSTALL.md`：用户把"按 <url>/INSTALL.md 安装 Distill"
发给 agent，agent 按步骤检查并安装 `distill` 二进制、运行 `distill init`（vault 位置问用户）、
添加 marketplace、安装插件、运行 `distill doctor`，最后提醒用户开一个新会话让插件生效。

插件和二进制分开发布，版本可能不一致。`distill mcp` 在 `serverInfo.version` 里报告版本，插件的
skill 写明最低版本要求，`distill doctor` 检查两者是否匹配。

MCP 工具（初版）：

| 工具 | 作用 |
|---|---|
| `distill_save` | 保存 note，参数是 note 格式（[data-model.md](data-model.md)）+ source；tag 分 `tags` 和 `new_tags` 两个字段，见下文 |
| `distill_recall` | 给一个问题，返回相似的已有 note 和标注、所属 topic 的提问次数，以及全部已有 tag 及使用次数 |
| `distill_search` | 按关键词、tag、时间段、项目检索 note |
| `distill_stats` | 按 tag / 项目 / 周 统计，返回 JSON，供 agent 分析 |

选 MCP 而不是让 skill 直接跑 shell：参数有类型，不依赖 agent 的 shell 权限，而且 `recall` /
`stats` 让 agent 能把 Distill 当作 context 来源（需求 2 的后半句）。CLI 仍然全部可用，
MCP 不可用的环境可以回退到 shell。

**tag：自由 tag，agent 打，但新建前必须先查已有的**（2026-09-13 讨论确定）。

- agent 先调 `distill_recall`，返回里带着全部已有 tag 和各自的使用次数。个人数据的 tag 量级是
  几十到几百个，全量返回没有问题。
- skill 要求：优先用已有 tag，没有合适的才新建；每条 note 1–5 个 tag。
- 协议上强制区分：`distill_save` 的 `tags` 只能填已有 tag，填了不存在的直接报错；要新建的必须
  放进 `new_tags`。agent 没法"顺手"造一个新 tag。
- 服务端再兜一层：`new_tags` 里的每个 tag 先做规范化（去首尾空格、英文转小写、全角转半角、
  空格和下划线统一成 `-`）。规范化后和已有 tag 相同的直接复用；拼写很接近的（比如 `sqlite`
  和 `sqlite3`，按编辑距离判断）或命中别名表的，拒绝保存，报错里列出候选，让 agent 重新决定：
  用已有的，或者确认新建（`confirm_new: true`）。意思相同但拼写不同的（`db` 和 `database`、
  `数据库` 和 `database`）服务端识别不了，靠 agent 看已有 tag 列表自己判断。
- 事后整理（合并、改名）走 D2 的 `tag-aliases/` 别名文件，不改 note。

**每条 note 必须记录来源**（2026-09-13 讨论确定）：`agent`（`codex` 或 `claude-code`）和
`session_id` 是必填，用来生成打开原会话的深链（D6）；`cwd`、`git_repo` 一并记录，Claude Code
的兜底命令需要 `cwd`。

MCP 协议本身不传宿主会话的 id。两个 agent 的实际情况（2026-09-13 本机检查）：

| | MCP server 能否自己拿到 session id | 依据 |
|---|---|---|
| Claude Code | 能：启动时的环境变量 `CLAUDE_CODE_SESSION_ID` | 本会话里 Claude Code 拉起的 MCP server（Pen，pid 85608）环境里有这个变量，值等于当前会话 id。风险：`/clear` 之后会话 id 变了，MCP server 进程可能沿用旧环境变量，P2 验证 |
| Codex | 不能：MCP server 由 `codex app-server` 统一拉起，环境里没有 thread id | 本机 app-server（pid 13061）下的 MCP server 进程环境里没有相关变量 |

Codex 的 hook 能拿到 session id：本机已装插件的 `hooks.json` 用 `"type": "mcp_tool"` 在 `Stop`
等事件里调用 MCP 工具，参数写 `"session_id": "${session_id}"`。

所以统一走 D12 的提问 hook：hook 每次注入的上下文里带一行
`distill-source: <agent> <session-id> <cwd>`，skill 要求 agent 调 `distill_save` 时原样填入。
一套机制覆盖两个 agent。服务端再做校验，防止 agent 抄错或编造：

- 格式必须是 UUID；
- Claude Code：和 MCP server 环境里的 `CLAUDE_CODE_SESSION_ID` 比对，不一致时以 agent 传入的为准
  并记一条 warn（对应 `/clear` 之后环境变量过期的情况）；
- Codex：检查 `~/.codex/sessions/` 下存在以这个 id 结尾的 rollout 文件；
- 缺失或校验失败时拒绝保存，报错说明"插件 hook 没有生效，运行 `distill doctor` 检查"，而不是
  存一条打不开原会话的 note。

备选：Codex 的 hook 用 `mcp_tool` 类型直接调 `distill_bind_session` 把 id 告诉 MCP server。前提是
每个 Codex 会话有自己的 MCP server 实例，否则多个会话同时开着时会绑错。P2 验证后再决定要不要用。

### D6. 来源适配器：只管"当前会话是谁"

不回填历史，所以不解析 JSONL。每个 agent 一个很薄的 `SourceAdapter`，只做两件事：

- 识别当前会话：agent 类型、session id、cwd、git 仓库、原始会话文件路径，写进 note 的来源，
  以后 Web UI 能跳回原会话；
- 把各家 hook 的输入统一成 agent 类型、session id、cwd，供 hook 生成注入内容（D12）。

不读 `state_5.sqlite` 这类 Codex 私有库，文件名里的版本号说明它会变。

**跳回原会话。** Web UI 在 note 上给出"打开原会话"：优先用桌面 App 的深链，同时总是显示一条
可复制的命令作为兜底。深链来自两个 App 的安装包（2026-09-13 本机检查，Codex App 26.915、
Claude 桌面 2.9939）：

| agent | 深链 | 兜底命令 |
|---|---|---|
| Codex | `codex://threads/<thread-id>`。App 自己生成"打开会话"链接时就用这个格式 | `codex resume <id>` |
| Claude Code | `claude://resume?session=<uuid>`。处理函数的日志写着 "Resume deep link: importing CLI session"，会把 CLI 会话导入桌面端再打开；另有 `claude://code/continue?session=<id>` 用于桌面端自己的会话 | `cd <cwd> && claude --resume <id>`（Claude Code 按项目目录存会话，要先进原目录） |

2026-09-13 用户用本机真实会话实测，两个深链都能打开对应会话。

这些 URL scheme 都不是公开文档里的接口，随时可能改。所以：链接的拼法集中在各自的
`SourceAdapter` 里，一处修改；兜底命令始终显示；Web UI 不去探测 App 是否安装，点了没反应时
用户还有命令可用。P2 的集成测试里保留一条手动检查：用 hook 拿到的 session id 拼出深链，确认
能打开。

### D7. 界面：CLI 内嵌本机 Web，不做桌面端

v1 的交付物只有一个 `distill` 可执行文件，`distill ui` 在本机回环地址起服务并打开浏览器
（2026-09-13 讨论确定）。

为什么不做桌面端：Distill 的界面是浏览 note、看统计、在 tag 和 topic 之间跳转，都不需要
原生能力。常被当成桌面端理由的几件事，CLI 都能做：

| 需求 | 做法 |
|---|---|
| 安装插件 | 用户在 agent 里自己装，见 D5 |
| 后台任务 | 在 `distill ui` 进程里跑，不依赖各平台的服务管理器 |
| 从 agent 跳到某条 note | agent 输出 `http://distill.localhost:<port>/notes/<id>`，终端里可点击 |
| 多设备同步 | vault 放进同步目录（D2），不需要带 iCloud entitlement 的签名 App |
| 独立窗口 | Chrome / Edge 的"安装为应用" |

省掉的：Electron / Tauri 选型、Rust 层、sidecar 编译、签名公证、自动更新、DMG。

运行时和分发方式见 D10。

**固定地址：`http://distill.localhost:<固定端口>`。** 地址一旦确定就不再变，agent 写进 note
或终端里的链接长期有效。

- **域名。** `*.localhost` 按标准解析到本机回环地址，不需要改 `/etc/hosts`。本机实测
  （macOS，2026-09-13）系统解析器把 `distill.localhost` 解析到 `::1`，所以服务要同时监听
  `127.0.0.1` 和 `::1`。Chrome、Edge、Firefox 自己解析 `*.localhost`，不依赖系统；Linux 和
  Windows 上的实际表现在 P3 验证。浏览器把 `localhost` 及其子域视为安全上下文，不需要 HTTPS。
- **端口只选一次。** `distill init` 时选定：默认端口空闲就用它，被占用就另选一个空闲端口，
  写进本机配置，之后永远用这个。不在每次启动时找空闲端口。
- **服务在需要时拉起，不注册系统服务。** 跨平台，所以不用 launchd、systemd、Windows 计划
  任务（2026-09-13 讨论确定）。两个拉起时机：
  - 用户执行 `distill ui`：前台运行并打开浏览器；
  - `distill mcp` 要给 agent 返回链接时：先检查服务是否在跑，不在就以后台进程启动
    `distill ui --background`，再返回链接。agent 会话开着时 `distill mcp` 一定在，所以 agent
    给出的链接点开时服务基本都在。
  后台服务一直运行到 `distill ui stop` 或用户注销，进程很轻，不做空闲自动退出，否则过一阵
  再点旧链接会打不开。
- **单实例。** 数据目录里放一个锁文件，记录 pid 和端口。启动前先请求 `/api/health` 确认对面
  是不是 Distill：是就复用；端口被别的程序占了就报错，尽量给出占用进程的名字和 pid（用跨平台的
  crate 查询），以及改端口的命令 `distill config set ui.port <port>`。不静默换端口，否则已经
  发出去的链接会全部失效。
- **端口可能被抢。** 服务没运行的时候，端口有可能被别的程序占掉，这时按上一条报错。以后如果
  真成问题，再按平台加可选的 `distill service install`，不进 v1。
- **兼容 portless。** 支持 `PORT` 环境变量和 `--port`，已经在用 portless 的人可以自己套一层。

不直接依赖 portless（2026-09-13 查询，pre-1.0）：它要 sudo 监听 443，要往系统信任区装一个
本地根证书，要常驻一个代理进程，还要求 Node 24。这些对个人开发工具合理，但作为 Distill 的
依赖，相当于让每个用户为了一个本地页面改系统安全设置。它解决的"固定域名、不撞端口"，用
上面的做法不需要 sudo 也能做到。也不监听 80 端口：本机实测非 root 可以绑定 `0.0.0.0:80`，
但那会把服务暴露给局域网；绑定 `127.0.0.1:80` 需要 root。

> **2026-09-26 实现备注（P3）。** 与上文不同的几处：
>
> - 没有单独的锁文件。`/api/health` 返回 `{"app": "distill", ...}`，启动前先问端口，它就是锁；两个
>   进程同时启动时，绑定失败的一方再问一次健康检查。
> - 后台启动用独立进程组（Unix `process_group(0)`，Windows `DETACHED_PROCESS`），不 fork 两次；
>   `distill mcp` 拉起前检查本机配置 `ui.autostart`（默认开），关掉后不拉起。
> - 一次性 token 和长期 secret 都放在数据目录 `ui/` 下，所以不管服务是谁拉起的，终端里的
>   `distill ui` 都能给浏览器授权。
> - 除了 cookie 和 Host，还校验 `Origin`：所有 `*.localhost` 端口都算同一个 site，`SameSite=Strict`
>   挡不住本机其他开发服务器带着 cookie 发 POST。非 GET 请求没有 `Origin` 时，只接受带 secret 的
>   CLI 请求。
>
> 现状见 `docs/web-ui.md`。

**localhost 安全必须做对。** 用户打开的任何网页都能向 localhost 发请求，所以：

- 只监听回环地址（`127.0.0.1` 和 `::1`）；
- 首次打开时 URL 带一次性 token，服务端换成 cookie，之后每个 API 请求都校验。cookie 绑定
  `distill.localhost` 这个主机名：cookie 不按端口隔离，如果挂在 `localhost` 上，本机其他开发
  服务器也能收到它。cookie 长期有效，所以 agent 给的链接不带 token 也能打开；浏览器里没有
  cookie 时，页面提示在终端执行 `distill ui` 授权。token 不写进 note 和链接，因为 vault 会同步；
- 校验 `Host` 头只接受 `distill.localhost`、`localhost`、`127.0.0.1`、`[::1]`，防 DNS rebinding；
- 不开 CORS。

**以后要桌面端时。** 出现菜单栏入口、全局快捷键、系统通知这类只有原生能做好的需求，再用
Tauri 把同一个 Web UI 装进窗口。core 是 Rust crate，可以直接链接进 Tauri，不需要 sidecar，
前端和 API 类型不变。

从 reflect-open 借这些：

- 前端通过可替换的 bridge 调 API，测试里换成假实现，以后进 Tauri 换成 `invoke`
  （reflect-open 的 `packages/core/src/ipc/bridge.ts`）；
- WAL 下读写分连接；
- 用内容 hash 跳过未变化的输入。

### D8. 隐私

没有遥测，不经过任何 Distill 自己的服务器。vault 放在同步目录时会上传到用户自己的云盘，
而对话里常有粘贴进去的 token、密钥，所以 note 和标注**写入文件之前**做一遍 secret 脱敏
（格式规则 + 高熵字符串）。以后接入任何外部 LLM 或 embedding API 时，参照 reflect-open 的
`private: true`：一个项目或一条 note 可以标记为永不外发，在检索层和 AI 边界两处强制执行。

### D9. Web 前端：React CSR 单页应用 + TanStack

标准的客户端渲染单页应用，由 `distill ui` 作为静态资源提供（2026-09-13 讨论确定）。

| 用途 | 选择 |
|---|---|
| 框架 | React 19 + React Compiler |
| 构建 | Vite |
| 路由 | TanStack Router（类型安全的路由和 search params，适合筛选条件放进 URL） |
| 数据 | TanStack Query，数据变化时由 SSE 事件触发失效 |
| 表格 | TanStack Table，只在列表需要排序、筛选时用 |
| 样式 | Tailwind v4，按 [distill-ui-design](../../.agents/skills/distill-ui-design/SKILL.md) 的设计规范 |
| 长列表 | `virtua` |
| 命令面板 | `cmdk` |

> **2026-09-26 实现备注（P3）。** 路由用代码定义，不生成 route tree；topic 列表只有几种固定排序，
> 没有引入 TanStack Table。字体用 `@fontsource` 拉丁子集随构建离线提供，中文回落到系统字体。

不需要 SSR：页面只在本机打开，首屏是从 localhost 读几百 KB 的静态文件。所以不用
TanStack Start 这类全栈框架，服务端只有 `distill ui` 一个。

### D10. 实现：Rust，单个静态二进制

（2026-09-13 讨论确定。）代码由 agent 写，所以评判标准是：agent 写出来的代码有多少错误能在
编译期被拦下，产物好不好分发，以后能不能直接复用。

**本机 spike（2026-09-13）**：`rusqlite`（`bundled`）+ `sqlite-vec` crate，release 构建：

| 指标 | 结果 |
|---|---|
| 二进制大小 | 2.3 MB，SQLite 3.53.2、FTS5、`sqlite-vec` v0.1.9 全部静态链接 |
| 启动 + 建库 + 查询 | 首次 0.55 s（Gatekeeper 检查），之后 < 10 ms |
| FTS5 `trigram` 中文 | 3 字查询命中，与 `node:sqlite` 一致 |
| 首次完整编译 | 22 s；增量编译几秒 |

对比过的方案：

| | Rust | Node + TS | Go |
|---|---|---|---|
| 分发 | 单文件，Web 构建产物用 `rust-embed` 一起嵌入 | npm 包，依赖 Node；`sqlite-vec` 是运行时加载的 `.dylib` | 单文件，但 `sqlite-vec` 要 cgo 或 wasm 版 SQLite |
| agent 写错的代价 | 类型、穷尽匹配、无 null，多数错误编译期暴露 | 运行时才暴露的更多 | 介于两者之间，nil 和错误忽略要靠 lint |
| 和 Web 共享类型 | `ts-rs` 生成 | 直接共用 zod | OpenAPI 生成 |
| hook 每次提问的开销 | 几 ms | 40 ms 起 | 几 ms |
| 以后做 Tauri | core 直接链接 | sidecar | sidecar |

选 Rust。Node 的最大优势是"前后端一种语言"，但前端和后端之间只隔着一组 API 类型，`ts-rs`
自动生成就能消除重复。Bun 在本机加载不了 `sqlite-vec`（"This build of sqlite3 does not
support dynamic extension loading"），已先行排除。Go 带上 `sqlite-vec` 就得用 cgo，失去它
最大的优势。

主要依赖（都是各自领域最常用、持续维护的库）：

| 用途 | crate |
|---|---|
| CLI 参数 | `clap` |
| SQLite | `rusqlite`（`bundled`）、`rusqlite_migration`、`sqlite-vec` |
| HTTP 与 SSE | `axum` |
| MCP | `rmcp`（官方 Rust SDK） |
| 文件监听 | `notify` |
| frontmatter | `serde` + `serde_yaml` 系 |
| 嵌入 Web 资源 | `rust-embed` |
| TS 类型生成 | `ts-rs` |
| ID | `ulid` |

仓库布局：

```
crates/distill-core/     vault、索引、检索、统计、脱敏
crates/distill-cli/      二进制 `distill`：子命令、`mcp`、`ui`
apps/web/                React + Vite（D9），pnpm 管理
plugins/distill/         Codex / Claude Code 插件（D5）
```

**分发**：现在 `cargo install --path crates/distill-cli`；以后用 GitHub Releases 发 macOS
（arm64 / x64）、Linux（x64 / arm64）、Windows（x64）的二进制，再加 Homebrew tap 和 Windows 的
winget / Scoop。CI 在三个平台上都跑测试。

**hook 和 MCP 的启动路径**：插件由用户自己安装（D5），`distill` 不改 agent 的配置，插件里也就
没法写死二进制的绝对路径。而桌面 App 启动 hook、MCP 时，PATH 里不一定有 `~/.cargo/bin`、
`/opt/homebrew/bin`。所以插件带一个很小的启动脚本，按顺序找 `distill`：`distill init` 记下的
自身路径（写在本机配置里）→ PATH → 常见安装位置。找不到时：hook 静默退出，绝不影响用户提问；
MCP 工具返回错误，说明如何安装。

> **2026-09-29 实现备注。** 从 marketplace 装插件的用户拿不到二进制：marketplace 分支只有插件，
> 也没有任何预编译发布，只能从源码构建。改为：打 `v<version>` tag 时 CI 把 Apple Silicon Mac、
> Linux arm64 / x86_64 的二进制发到 GitHub Releases（没有 Intel Mac：ort-sys 不提供它的 ONNX
> Runtime 预编译库，源码也编不过）；插件构建把 workspace 版本写进
> `bin/distill-version`，启动脚本找不到 `distill` 时由 MCP server 下载这个版本并校验 SHA-256，
> 放在本机数据目录。hook 仍然不联网。考虑过的另两条路：二进制直接提交进 marketplace 分支（每个
> 平台约 44 MB，每次发布都进 git 历史，分支会越来越大）；只提供安装脚本（多一步手动操作，插件
> 装完仍然不能用）。Windows、Homebrew tap 仍未做。现状见 `docs/plugin.md`。

**对仓库规则的影响**：`AGENTS.md` 已相应更新（仓库结构、索引迁移、API 类型生成、版本字段、
Rust 的错误处理写法）。`mise run check` 要同时覆盖 `cargo fmt --check`、
`cargo clippy -D warnings`、`cargo test` 和前端检查，在 P1 搭仓库时写进 `mise.toml`。

### D11. 用户的理解靠 Web UI 标注，不在 distill 时追问

distill 时 agent 只生成 note，不要求用户写自己的理解；用户之后在 Web UI 里给 note 加标注
（2026-09-13 讨论确定）。distill 保持一步完成，不打断当前会话。

- **存储。** 每条标注是 vault 里单独的一个 Markdown 文件：
  `annotations/<note-id>/<annotation-id>.md`，frontmatter 记 `id`、`note`、`created`、`updated`，
  正文是标注内容。不改 note 文件，符合 D2 的不冲突写法；只有同一条标注在两台设备上同时被改
  时才会冲突，按 D2 的冲突副本处理。
- **编辑和删除。** 改标注就是改它自己的文件，删除就是删文件。
- **和"重问"的关系。** 重问命中某个 topic 时，`distill_recall` 连同标注一起返回，agent 能看到
  你上次自己的理解，而不只是上次 AI 的结论。统计里可以区分"有标注"和"没标注"的 topic。
- **标注的粒度**见 Plan 的"推迟到后续阶段"。

### D12. 提问 hook 只做提示；agent 的行为规则全部写在插件的 skill 里

插件带一个提问提交时触发的 hook。它只往上下文里注入几行短提示，不写任何文件、不碰索引，
也不记录提问原文。什么时候查 Distill、什么时候建议 distill、怎么 distill，全部写在插件的
`distill` skill 里，hook 只提醒 agent 去读它（2026-09-13 讨论确定）。规则只有 skill 这一个家，
调整行为改 skill 即可，不用动二进制和 hook。

**两个 agent 都支持。** Claude Code 有 `UserPromptSubmit`。Codex 插件也支持：本机已安装的
`language-coach` 插件在 `hooks/hooks.json` 里注册了 `UserPromptSubmit`，命令里用
`${PLUGIN_ROOT}` 引用插件目录，从 stdin 读 JSON（它读了 `turn_id`），通过 stdout 输出
`hookSpecificOutput.additionalContext` 注入上下文，和 Claude Code 的约定一致。Codex stdin 里
session id 的字段名，P2 先把 stdin 打出来确认；hook 上下文里有 session id 已经确认（见 D5）。

**hook 注入的内容**（`additionalContext`），不调 LLM，Rust 二进制几毫秒完成：

```
distill-source: <agent> <session-id> <cwd>
distill-suggest: on
Distill 已安装。本会话还没读过 distill skill 的话，先读它，按其中的规则决定何时查询和建议 distill。
```

- `distill-source` 供 D5 的来源记录使用。
- `distill-suggest` 反映本机配置 `suggest.enabled`；为 `off` 时 skill 规定不主动建议 distill，
  其余行为不变。
- 最后一行只是指路，不重复 skill 里的任何规则。"本会话还没读过才读"避免每次提问都重读。

**skill 的内容**（`plugins/distill/skills/distill/SKILL.md`，P2 编写；note 格式等细节放
`references/`）：

| 时机 | skill 规定的行为 |
|---|---|
| 回答前 | 遇到概念、原理、方案、排错根因这类问题，先调 `distill_recall` 看以前是否问过。命中时把上次的结论和用户标注带进回答，并告诉用户"这是第 N 次问，上次的 note 在 <链接>"。写代码、跑命令这类指令不查 |
| 回答后 | `distill-suggest: on` 且这一轮值得以后回看时，在末尾问一次是否 distill。值得的：解释概念或原理、带取舍的决策、排查出根因的问题、用户追问了几轮的问题。不值得的：一次性命令查询、简单改代码、闲聊。同一会话里同一个问题只问一次，用户拒绝后不再追问 |
| distill 时 | 用户同意或主动发起（`/distill`）后：调 `distill_recall` → 判断是否已有 topic、选 tag（D5 的 tag 规则）→ 按 note 格式整理 → 调 `distill_save`，原样填入 `distill-source` → 把链接和"第 N 次问"告诉用户 |
| 用户要分析时 | 用 `distill_stats` / `distill_search` / `distill_recall` 取数据再分析（需求 2） |

## 数据模型

vault 文件布局、note 文件格式和索引表见 [data-model.md](data-model.md)。

## Rejected

- **Electron**：空壳就要 150–250 MB。
- **v1 做桌面端（Tauri）**：界面不需要原生能力，推迟到出现菜单栏、全局快捷键这类需求时，见 D7。
- **线上托管的 Web**：要把对话数据上传服务器，违背全部本地的前提。
- **Octane**：2026-06 才创建，0.5.0 beta，发版频繁；在 Distill 的数据量下性能差异感知不到，
  换不回它的新鲜度风险。
- **TanStack Start / SSR**：只在本机打开，不需要服务端渲染，见 D9。
- **Bun**：`bun:sqlite` 在 macOS 上加载不了 `sqlite-vec`，见 D10。
- **Node + TS / Go**：见 D10 的对比。
- **PGlite**：单进程，见 D2。
- **独立向量数据库**：在这个数据量下只增加一份存储和一致性负担，见 D3。
- **回填历史会话**：从安装时开始记录就够了，重问会在之后的使用中自然出现。省掉 JSONL 解析、
  格式兼容和大批量打 tag。
- **界面每次请求 spawn CLI、解析 stdout**：换成长驻的 `distill ui` 服务，见 D1。
- **把原始 JSONL 复制进 Distill**：本机 Codex 会话已有 1.0 GB，且原文件就在本地。只存路径，
  需要原文时回源读。
- **Rust 持有数据、TS 手写一份 zod 对应**：reflect-open 自己列为痛点；这里 TS 类型全部由
  `ts-rs` 生成，见 D1。
- **读 Codex 的内部 SQLite**：私有、带版本号，见 D6。
- **SQLite 作为真实来源**：没法靠 iCloud 同步，正在使用的数据库文件放进同步文件夹会损坏，见 D2。
- **索引放在 vault 里、用 xattr 排除同步**：reflect-open 这样做后遇到 iCloud 反复扫描、
  CPU 长期占用的问题，见 D2。
- **自动三方合并冲突**：写法规则保证基本不冲突，剩下的少数情况交给用户选，见 D2。
- **记录每条提问原文**：唯一的独有价值是统计"问了但没 distill"的次数。代价是用户输入的每句话
  都随 vault 同步到云盘，其中大量是写代码的指令，还可能夹着脱敏拦不住的密钥；做相似匹配时这些
  指令又是主要噪音。有了 D12 的主动建议，盲区已经小了。以后确实需要时，考虑只记录"建议了但
  用户拒绝"的问题，由 agent 记下整理后的一句话，不记原文。

## Open questions

全部已决定或推迟到对应阶段，见下方 Plan 的"推迟到后续阶段"。讨论中已决定的问题编号保留在
正文引用处：Q1 → D12，Q2 → D2，Q3 → D11，Q4 → D5，Q6 → D7、D10，Q7 / Q10 → D10，
Q9 → D12，Q12 → D6，Q14 → 下面这条：

- **同一个问题的判定流程**（原 Q14，2026-09-13 讨论确定）：skill 要求 agent 先调
  `distill_recall` 拿候选，由 agent 判断是否属于已有 topic，在 `distill_save` 里给出 `topic`
  （已有 topic id 或 `new`）。tag 按 D5 单独选。CLI 手动保存时用 `--topic`。

## Plan

| 阶段 | 内容 | 退出标准 |
|---|---|---|
| P1 Core + CLI | `distill-core`、`distill-cli`，`init` / `save` / `recall` / `search` / `stats` / `reindex`，全部支持 `--json` | `distill save` 两条同义问题后，`distill stats --json` 显示同一 topic 提问 2 次 |
| P2 插件 | `distill mcp`、skill、两个 manifest、提问 hook（D12） | 在 Codex 和 Claude Code 里各 `/distill` 一次，note 入库并关联来源 session；一次值得记录的问答结束后 agent 主动问要不要 distill；第二次问同一问题时 agent 提示"第 2 次" |
| P3 Web UI | `distill ui`、localhost 安全措施、React 前端（D7、D9），浏览 topic / note / tag / 统计 | 从 agent 输出的链接打开一条 note，在页面里从它的 tag 跳到同 tag 的其他 note，再跳回原始会话 |
| P4 语义层 | embedding 提高召回、topic 合并与命名 | recall 能找回换了说法、FTS 找不到的同一个问题 |

### 推迟到后续阶段

| 问题 | 在哪个阶段前定 | 目前倾向 |
|---|---|---|
| Web UI 除了 annotation 还能改什么（原 Q11） | P3 | 能处理冲突副本、合并 topic；note 正文和 tag 用自己的编辑器改 |
| 统计页 v1 必须有什么（原 Q13） | P3 | 候选：每周 distill 数、按 tag / 项目的分布、重问最多的 topic、没有 annotation 的 topic、每个 topic 的时间线 |
| annotation 的粒度（原 Q15） | P3 | 先做整条 note 级别，格式里预留可选的 `anchor` 字段 |
| 后台任务的 LLM 和 embedding 从哪来（原 Q5） | P4 | 本地多语 embedding + 无头 agent（`codex exec` / `claude -p`）做 topic 命名和 tag 整理 |
| 更多来源（原 Q8） | 需要时 | ChatGPT / Claude.ai 导出、Cursor、Gemini CLI；只影响 `SourceAdapter` 接口 |

> **2026-09-26 决定（P3、P4 开工时）。**
>
> - **Q11**：按倾向做。Web UI 能增删改 annotation、给 topic 改名、合并 topic、处理冲突副本。保留
>   冲突中的一份时，其余副本移到本机数据目录 `discarded/<vault-id>/`，不删除。note 正文和 tag
>   仍用编辑器改。
> - **Q13**：候选全做。统计页有每周 note 数（最近 16 周）、按 tag / 项目的分布、重问最多的
>   topic、重问过却没有 annotation 的 topic；topic 的时间线放在 topic 页。
> - **Q15**：只做整条 note 的 annotation，`anchor` 继续保留不写。
> - **Q5**：embedding 见 D3 的 2026-09-26 备注。topic 命名与合并**不跑后台 LLM**：topic 的名字
>   在 Web UI 手动改，embedding 找出「可能是同一个问题」的 topic 对，在 Web UI 列出来，由用户一键
>   合并或标记「不是同一个问题」（标记只存在本机索引里）。无头 agent 会消耗用户的 agent 额度、
>   结果不确定，而命名只影响可读性、不影响提问次数，不值得。tag 整理仍靠 D5 的别名文件，没有自动化。

P2 用起来 2–4 周后回看一次数据：重问是否真的频繁、匹配是否准。这是对整个产品假设的验证，
结果决定 P3、P4 的优先级。

> **2026-09-26 进展。** P3、P4 已实现。P4 退出标准由 `semantic.rs` 覆盖（需先拉取模型）；P3 的
> 授权、note 页、tag 跳转已在本机浏览器走过。未验证：真实会话里点开 agent 给的链接并深链跳回
> 原会话；Linux、Windows 上 `*.localhost` 的解析。
