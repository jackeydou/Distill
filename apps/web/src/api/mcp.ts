import type { App } from "@modelcontextprotocol/ext-apps";
import { ApiError, type Bridge, type Method } from "./bridge";
import type { McpResponse } from "./generated/McpResponse";

export function createMcpBridge(app: Pick<App, "callServerTool">): Bridge {
  return {
    async request<T>(method: Method, path: string, body?: unknown): Promise<T> {
      const result = await app.callServerTool({
        name: method === "GET" ? "distill_ui_read" : "distill_ui_write",
        arguments:
          method === "GET" ? { path } : { method, path, ...(body === undefined ? {} : { body }) },
      });
      if (result.isError) {
        throw new Error(
          result.content
            .filter((c) => c.type === "text")
            .map((c) => c.text)
            .join("\n"),
        );
      }
      const response = result.structuredContent as McpResponse | undefined;
      if (!response || typeof response.status !== "number" || !("data" in response)) {
        throw new Error(`Invalid Distill MCP response for ${method} ${path}`);
      }
      if (response.status >= 400) {
        const data = response.data as { error?: unknown } | null;
        throw new ApiError(
          response.status,
          typeof data?.error === "string" ? data.error : `UI API ${response.status} for ${path}`,
        );
      }
      return (response.status === 204 ? undefined : response.data) as T;
    },
    subscribe(refresh) {
      // MCP Apps have no HTTP event stream; refetch while visible and on return to the tab.
      const onTick = () => {
        if (document.visibilityState === "visible") refresh();
      };
      const timer = window.setInterval(onTick, 5000);
      window.addEventListener("focus", refresh);
      document.addEventListener("visibilitychange", onTick);
      return () => {
        window.clearInterval(timer);
        window.removeEventListener("focus", refresh);
        document.removeEventListener("visibilitychange", onTick);
      };
    },
  };
}
