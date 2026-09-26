import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { queryClient } from "./api/queries";
import { applyTheme } from "./components/ThemeToggle";
import { router } from "./router";
import "./styles.css";

applyTheme();

const container = document.getElementById("root");
if (!container) {
  throw new Error("index.html has no #root element");
}
createRoot(container).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
