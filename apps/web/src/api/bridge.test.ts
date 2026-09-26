import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, httpBridge } from "./bridge";
import { queryString } from "./client";

afterEach(() => vi.unstubAllGlobals());

function respond(status: number, body: string) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response(status === 204 ? null : body, { status })),
  );
}

describe("httpBridge", () => {
  it("returns parsed JSON", async () => {
    respond(200, '{"notes":3}');
    await expect(httpBridge.request("GET", "/stats")).resolves.toEqual({ notes: 3 });
  });

  it("turns the server's error body into an ApiError", async () => {
    respond(404, '{"error":"unknown note 01X."}');
    const err = await httpBridge.request("GET", "/notes/01X").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(404);
    expect((err as ApiError).message).toBe("unknown note 01X.");
  });

  it("keeps non-JSON error text", async () => {
    respond(502, "Bad Gateway");
    await expect(httpBridge.request("GET", "/stats")).rejects.toThrow("Bad Gateway");
  });

  it("sends JSON bodies and accepts 204", async () => {
    respond(204, "");
    await expect(httpBridge.request("PUT", "/topics/1", { label: "x" })).resolves.toBeUndefined();
    const call = vi.mocked(fetch).mock.calls[0];
    expect(call?.[0]).toBe("/api/topics/1");
    expect(call?.[1]?.body).toBe('{"label":"x"}');
  });
});

describe("queryString", () => {
  it("skips empty values", () => {
    expect(queryString({ q: "", tag: "sqlite", limit: 20, topic: undefined })).toBe(
      "?tag=sqlite&limit=20",
    );
    expect(queryString({})).toBe("");
  });
});
