import type { App } from "@modelcontextprotocol/ext-apps";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./bridge";
import { createMcpBridge } from "./mcp";

const result = (status: number, data: unknown) => ({
  content: [],
  structuredContent: { status, data },
});

function host(response: Awaited<ReturnType<App["callServerTool"]>>) {
  return { callServerTool: vi.fn(async () => response) };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("MCP bridge", () => {
  it("reads through the app-only tool and preserves API data", async () => {
    const app = host(result(200, { notes: 3 }));
    await expect(createMcpBridge(app).request("GET", "/stats")).resolves.toEqual({ notes: 3 });
    expect(app.callServerTool).toHaveBeenCalledWith({
      name: "distill_ui_read",
      arguments: { path: "/stats" },
    });
  });

  it("writes through the app-only tool and accepts empty responses", async () => {
    const app = host(result(204, null));
    await expect(createMcpBridge(app).request("DELETE", "/annotations/1")).resolves.toBeUndefined();
    expect(app.callServerTool).toHaveBeenCalledWith({
      name: "distill_ui_write",
      arguments: { method: "DELETE", path: "/annotations/1" },
    });
  });

  it("preserves API status for authentication and retry decisions", async () => {
    const app = host(result(404, { error: "unknown note 01X" }));
    const error = await createMcpBridge(app)
      .request("GET", "/notes/01X")
      .catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(404);
    expect((error as Error).message).toBe("unknown note 01X");
  });

  it("reports MCP errors and malformed host responses", async () => {
    const app = host({
      isError: true,
      content: [{ type: "text", text: "cannot open vault /missing" }],
    });
    await expect(createMcpBridge(app).request("GET", "/stats")).rejects.toThrow(
      "cannot open vault /missing",
    );
    app.callServerTool.mockResolvedValue({ content: [] });
    await expect(createMcpBridge(app).request("GET", "/stats")).rejects.toThrow(
      "Invalid Distill MCP response",
    );
  });

  it("refreshes visible apps and removes timers and listeners on close", () => {
    vi.useFakeTimers();
    const browser = new EventTarget();
    const page = Object.assign(new EventTarget(), { visibilityState: "visible" });
    vi.stubGlobal("window", Object.assign(browser, { setInterval, clearInterval }));
    vi.stubGlobal("document", page);
    const refresh = vi.fn();
    const close = createMcpBridge(host(result(200, {}))).subscribe(refresh);
    vi.advanceTimersByTime(5000);
    expect(refresh).toHaveBeenCalledTimes(1);
    page.visibilityState = "hidden";
    vi.advanceTimersByTime(5000);
    expect(refresh).toHaveBeenCalledTimes(1);
    browser.dispatchEvent(new Event("focus"));
    expect(refresh).toHaveBeenCalledTimes(2);
    close();
    vi.advanceTimersByTime(5000);
    browser.dispatchEvent(new Event("focus"));
    expect(refresh).toHaveBeenCalledTimes(2);
  });
});
