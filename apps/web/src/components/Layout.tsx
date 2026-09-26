import { useQuery } from "@tanstack/react-query";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { Search, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { ApiError } from "../api/bridge";
import { q, useVaultEvents } from "../api/queries";
import { CommandPalette } from "./CommandPalette";
import { ThemeToggle } from "./ThemeToggle";
import { cx, ErrorBox, Loading } from "./ui";

export function Layout() {
  const session = useQuery(q.session());
  const [paletteOpen, setPaletteOpen] = useState(false);
  useVaultEvents(session.isSuccess);

  if (session.error instanceof ApiError && session.error.status === 401) {
    return <AuthRequired />;
  }
  return (
    <div className="min-h-screen lg:grid lg:grid-cols-[16rem_minmax(0,1fr)]">
      <Sidebar onSearch={() => setPaletteOpen(true)} />
      <main className="min-w-0 px-4 pt-6 pb-24 sm:px-8 lg:px-12 lg:pt-10">
        <div className="mx-auto max-w-[64rem]">
          {session.isPending ? (
            <Loading />
          ) : session.isError ? (
            <ErrorBox error={session.error} />
          ) : (
            <Outlet />
          )}
        </div>
      </main>
      {session.isSuccess && <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />}
    </div>
  );
}

type Tab = "distill" | "review";

/** Review owns the dashboard and the problems page; everything else reads notes. */
function currentTab(pathname: string): Tab {
  return pathname.startsWith("/review") || pathname.startsWith("/problems") ? "review" : "distill";
}

function Sidebar({ onSearch }: { onSearch: () => void }) {
  const location = useRouterState({ select: (s) => s.location });
  const tab = currentTab(location.pathname);
  const activeTag =
    location.pathname === "/" && typeof location.search.tag === "string"
      ? location.search.tag
      : undefined;
  const problems = useQuery(q.problems());
  const problemCount =
    (problems.data?.conflicts.length ?? 0) + (problems.data?.invalid_files.length ?? 0);

  return (
    <aside className="flex flex-col border-b border-line bg-panel lg:sticky lg:top-0 lg:h-screen lg:border-r lg:border-b-0">
      <div className="flex items-center justify-between gap-2 px-4 pt-3 lg:px-5 lg:pt-6">
        <Link to="/" className="flex items-center gap-2 font-display text-xl text-ink">
          <span className="inline-block size-3 bg-brand" aria-hidden />
          Distill
        </Link>
        <div className="flex items-center gap-2 lg:hidden">
          <ThemeToggle />
          <SearchButton onClick={onSearch} compact />
        </div>
      </div>

      <nav aria-label="主导航" className="px-4 pt-4 lg:px-5">
        <div className="grid grid-cols-2 rounded-panel border border-line-strong p-0.5">
          <TabLink to="/" active={tab === "distill"}>
            Distill
          </TabLink>
          <TabLink to="/review" active={tab === "review"}>
            Review
          </TabLink>
        </div>
      </nav>

      <div className="hidden px-5 pt-3 lg:block">
        <SearchButton onClick={onSearch} />
      </div>

      <TagList activeTag={activeTag} />

      <div className="hidden items-center justify-between gap-2 border-t border-line px-4 py-3 lg:flex">
        <ThemeToggle />
        {problemCount > 0 && (
          <Link
            to="/problems"
            className="inline-flex items-center gap-1.5 rounded-[1px] px-2 py-1 text-xs text-error hover:bg-error-soft"
          >
            <TriangleAlert className="size-3.5" aria-hidden />
            {problemCount} 需要处理
          </Link>
        )}
      </div>
    </aside>
  );
}

function TabLink({
  to,
  active,
  children,
}: {
  to: "/" | "/review";
  active: boolean;
  children: string;
}) {
  return (
    <Link
      to={to}
      aria-current={active ? "page" : undefined}
      className={cx(
        "rounded-[1px] py-1.5 text-center text-sm transition-colors duration-150",
        active
          ? "bg-brand-strong text-on-brand"
          : "text-ink-muted hover:bg-panel-hover hover:text-ink",
      )}
    >
      {children}
    </Link>
  );
}

function SearchButton({ onClick, compact }: { onClick: () => void; compact?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label="搜索"
      className={cx(
        "flex h-8 items-center gap-2 rounded-[1px] border border-line text-sm text-ink-faint transition-colors duration-150 hover:border-line-strong hover:text-ink",
        compact ? "px-2" : "w-full px-2.5",
      )}
    >
      <Search className="size-4" aria-hidden />
      {!compact && (
        <>
          搜索
          <kbd className="ml-auto font-mono text-xs">⌘K</kbd>
        </>
      )}
    </button>
  );
}

/** Every tag, most used first. Choosing one filters the Distill timeline. */
function TagList({ activeTag }: { activeTag: string | undefined }) {
  const tags = useQuery(q.tags());
  const items = tags.data ?? [];
  return (
    <section aria-label="Tags" className="min-h-0 lg:flex lg:flex-1 lg:flex-col lg:pt-6">
      <h2 className="hidden px-5 pb-2 font-body text-[11px] tracking-wider text-ink-faint uppercase lg:block">
        Tags
      </h2>
      <ul className="flex gap-1.5 overflow-x-auto px-4 py-3 lg:block lg:flex-1 lg:space-y-px lg:overflow-y-auto lg:px-3 lg:py-0 lg:pb-4">
        {items.map((t) => {
          const active = t.tag === activeTag;
          return (
            <li key={t.tag} className="shrink-0">
              <Link
                to="/"
                search={active ? {} : { tag: t.tag }}
                aria-current={active ? "true" : undefined}
                className={cx(
                  "flex items-center gap-2 rounded-[1px] border px-2 py-1 text-sm transition-colors duration-150 lg:border-0",
                  active
                    ? "border-brand bg-brand-soft text-brand-ink"
                    : "border-line text-ink-muted hover:bg-panel-hover hover:text-ink",
                )}
              >
                <span className="truncate">#{t.tag}</span>
                <span className="ml-auto text-xs text-ink-faint tabular-nums">{t.notes}</span>
              </Link>
            </li>
          );
        })}
        {tags.isSuccess && items.length === 0 && (
          <li className="px-2 text-sm text-ink-faint">还没有 tag</li>
        )}
      </ul>
    </section>
  );
}

function AuthRequired() {
  return (
    <div className="flex min-h-screen items-center justify-center px-4">
      <div className="max-w-md rounded-panel border border-line bg-panel p-8">
        <h1 className="text-2xl">这个浏览器还没有授权</h1>
        <p className="mt-4">在终端运行下面的命令，它会打开一个已授权的页面：</p>
        <pre className="mt-3 border border-line bg-card px-3 py-2 font-mono text-sm text-ink">
          distill ui
        </pre>
        <p className="mt-4 text-sm text-ink-faint">
          授权只需一次，之后这个浏览器打开 Distill 的链接都能直接看到内容。
        </p>
      </div>
    </div>
  );
}
