# @distill/web

The web UI that `distill ui` serves. A React single-page app built by Vite into
`apps/web/dist`, which `crates/distill-cli` embeds at compile time. It never touches the vault
or index; everything goes through the JSON API in [docs/web-ui.md](../../docs/web-ui.md).

## Stack

React 19 with the React Compiler, TanStack Router (code-based routes in `src/router.tsx`) and
TanStack Query, Tailwind v4, `virtua` for long lists, `cmdk` for the ⌘K palette,
`react-markdown` with GFM for note bodies. Fonts come from `@fontsource` packages (IBM Plex
Serif, iA Writer Quattro, Lilex; Latin subsets) and ship inside the bundle, so the UI works
offline. Chinese text falls back to the system's CJK fonts.

## Layout

| Path | Holds |
|---|---|
| `src/api/bridge.ts` | The only transport. `setBridge` swaps it in tests |
| `src/api/client.ts` | One typed function per API route |
| `src/api/queries.ts` | Query keys, the query client, the SSE subscription that refetches on change |
| `src/api/generated/` | TypeScript API types generated from Rust by ts-rs. Never edit by hand |
| `src/pages/` | One component per route |
| `src/components/` | Shared pieces: layout, note list, annotations, charts, command palette |
| `src/lib/` | Formatting and ISO-week helpers, with tests |
| `src/styles.css` | Design tokens (semantic color roles, fonts) and note prose styles |

Design direction and token rules: the
[distill-ui-design skill](../../.agents/skills/distill-ui-design/SKILL.md).

## Pages

| Route | Shows |
|---|---|
| `/` | Most-asked topics, repeated topics without an annotation, recent notes, a banner when files need attention |
| `/notes?q=` | Keyword search, or recent notes |
| `/notes/$noteId` | The note, its topic and ask count, tags, annotations (add, edit, delete), how to reopen the session |
| `/topics?view=asked\|recent\|undigested` | Every topic |
| `/topics/$topicId` | Timeline of the topic's notes; rename; merge into another topic |
| `/tags`, `/tags/$tag` | Tags by note count; notes with a tag |
| `/stats` | Counts, notes per ISO week (16 weeks), most-asked and unannotated topics, tags, projects |
| `/problems` | Sync conflicts (keep one copy) and unreadable files |

## Scripts

Run from the repo root with `pnpm -C apps/web <script>`, or through `mise run check`.

| Script | Does |
|---|---|
| `dev` | Vite dev server; proxies `/api` and `/auth` to `distill ui` on `DISTILL_UI_PORT` (default 4777) |
| `build` | Writes `dist/` |
| `typecheck` / `lint` / `test` | `tsc`, Biome, Vitest |

With `pnpm dev`, authorize the dev origin by opening the link `distill ui --no-open` prints
with its host replaced by `localhost:5173`.
