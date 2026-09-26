import { useQuery } from "@tanstack/react-query";
import { Link, Outlet } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { type ReactNode, useState } from "react";
import { ApiError } from "../api/bridge";
import { q, useVaultEvents } from "../api/queries";
import { CommandPalette } from "./CommandPalette";
import { ThemeToggle } from "./ThemeToggle";
import { ErrorBox, Loading } from "./ui";

export function Layout() {
  const session = useQuery(q.session());
  const [paletteOpen, setPaletteOpen] = useState(false);
  useVaultEvents(session.isSuccess);

  if (session.error instanceof ApiError && session.error.status === 401) {
    return <AuthRequired />;
  }
  return (
    <div className="min-h-screen lg:grid lg:grid-cols-[15rem_minmax(0,1fr)]">
      <Sidebar onSearch={() => setPaletteOpen(true)} />
      <main className="min-w-0 px-4 pt-6 pb-24 sm:px-8 lg:px-12 lg:pt-12">
        <div className="mx-auto max-w-[52rem]">
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

function Sidebar({ onSearch }: { onSearch: () => void }) {
  const problems = useQuery(q.problems());
  const problemCount =
    (problems.data?.conflicts.length ?? 0) + (problems.data?.invalid_files.length ?? 0);
  return (
    <aside className="border-b border-line bg-panel lg:sticky lg:top-0 lg:h-screen lg:border-r lg:border-b-0">
      <div className="flex items-center justify-between gap-2 px-4 py-3 lg:px-5 lg:pt-6 lg:pb-5">
        <Link to="/" className="flex items-center gap-2 font-display text-xl text-ink">
          <span className="inline-block size-3 bg-brand" aria-hidden />
          Distill
        </Link>
        <div className="flex items-center gap-2 lg:hidden">
          <ThemeToggle />
          <button
            type="button"
            onClick={onSearch}
            className="inline-flex h-8 items-center gap-2 rounded-[1px] border border-line px-2 text-sm text-ink-faint transition-colors duration-150 hover:border-line-strong hover:text-ink"
            aria-label="搜索"
          >
            <Search className="size-4" aria-hidden />
          </button>
        </div>
      </div>
      <button
        type="button"
        onClick={onSearch}
        className="mx-5 mb-4 hidden h-9 w-[calc(100%-2.5rem)] items-center gap-2 rounded-[1px] border border-line px-2.5 text-sm text-ink-faint transition-colors duration-150 hover:border-line-strong hover:text-ink lg:flex"
      >
        <Search className="size-4" aria-hidden />
        搜索
        <kbd className="ml-auto font-mono text-xs">⌘K</kbd>
      </button>
      <nav
        aria-label="主导航"
        className="flex gap-1 overflow-x-auto px-3 pb-2 lg:flex-col lg:gap-0.5 lg:px-4 lg:pb-0"
      >
        <NavItem to="/">首页</NavItem>
        <NavItem to="/notes">Notes</NavItem>
        <NavItem to="/topics">Topics</NavItem>
        <NavItem to="/tags">Tags</NavItem>
        <NavItem to="/stats">统计</NavItem>
        {problemCount > 0 && (
          <NavItem to="/problems">
            需要处理
            <span className="ml-1.5 rounded-[1px] bg-error px-1 text-xs text-on-brand tabular-nums">
              {problemCount}
            </span>
          </NavItem>
        )}
      </nav>
      <div className="hidden px-4 lg:absolute lg:bottom-4 lg:block">
        <ThemeToggle />
      </div>
    </aside>
  );
}

function NavItem({ to, children }: { to: string; children: ReactNode }) {
  return (
    <Link
      to={to}
      activeOptions={{ exact: to === "/" }}
      className="flex shrink-0 items-center py-1 text-sm lg:py-0.5"
    >
      {({ isActive }) => (
        <span
          className={
            isActive
              ? "rounded-[1px] bg-brand-strong px-2 py-1 text-on-brand"
              : "rounded-[1px] px-2 py-1 text-ink-muted transition-colors duration-150 hover:text-ink"
          }
        >
          {children}
        </span>
      )}
    </Link>
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
