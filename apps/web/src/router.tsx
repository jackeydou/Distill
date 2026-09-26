import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Layout } from "./components/Layout";
import { Empty } from "./components/ui";
import { HomePage } from "./pages/HomePage";
import { NotePage } from "./pages/NotePage";
import { NotesPage } from "./pages/NotesPage";
import { ProblemsPage } from "./pages/ProblemsPage";
import { StatsPage } from "./pages/StatsPage";
import { TagPage, TagsPage } from "./pages/TagsPage";
import { TopicPage } from "./pages/TopicPage";
import { TopicsPage, type TopicView } from "./pages/TopicsPage";

const root = createRootRoute({
  component: Layout,
  notFoundComponent: () => <Empty>这个页面不存在。</Empty>,
});

const TOPIC_VIEWS: TopicView[] = ["asked", "recent", "undigested", "duplicates"];

const routeTree = root.addChildren([
  createRoute({ getParentRoute: () => root, path: "/", component: HomePage }),
  createRoute({
    getParentRoute: () => root,
    path: "/notes",
    component: NotesPage,
    validateSearch: (search: Record<string, unknown>): { q?: string } => ({
      q: typeof search.q === "string" && search.q ? search.q : undefined,
    }),
  }),
  createRoute({ getParentRoute: () => root, path: "/notes/$noteId", component: NotePage }),
  createRoute({
    getParentRoute: () => root,
    path: "/topics",
    component: TopicsPage,
    validateSearch: (search: Record<string, unknown>): { view?: TopicView } => ({
      view: TOPIC_VIEWS.find((v) => v === search.view),
    }),
  }),
  createRoute({ getParentRoute: () => root, path: "/topics/$topicId", component: TopicPage }),
  createRoute({ getParentRoute: () => root, path: "/tags", component: TagsPage }),
  createRoute({ getParentRoute: () => root, path: "/tags/$tag", component: TagPage }),
  createRoute({ getParentRoute: () => root, path: "/stats", component: StatsPage }),
  createRoute({ getParentRoute: () => root, path: "/problems", component: ProblemsPage }),
]);

export const router = createRouter({ routeTree, scrollRestoration: true });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
