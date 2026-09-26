import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Layout } from "./components/Layout";
import { Empty } from "./components/ui";
import { DistillPage } from "./pages/DistillPage";
import { NotePage } from "./pages/NotePage";
import { ProblemsPage } from "./pages/ProblemsPage";
import { ReviewPage } from "./pages/ReviewPage";
import { TopicPage } from "./pages/TopicPage";

const root = createRootRoute({
  component: Layout,
  notFoundComponent: () => <Empty>这个页面不存在。</Empty>,
});

const routeTree = root.addChildren([
  createRoute({
    getParentRoute: () => root,
    path: "/",
    component: DistillPage,
    validateSearch: (search: Record<string, unknown>): { tag?: string } => ({
      tag: typeof search.tag === "string" && search.tag ? search.tag : undefined,
    }),
  }),
  createRoute({ getParentRoute: () => root, path: "/review", component: ReviewPage }),
  createRoute({ getParentRoute: () => root, path: "/notes/$noteId", component: NotePage }),
  createRoute({ getParentRoute: () => root, path: "/topics/$topicId", component: TopicPage }),
  createRoute({ getParentRoute: () => root, path: "/problems", component: ProblemsPage }),
]);

export const router = createRouter({ routeTree, scrollRestoration: true });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
