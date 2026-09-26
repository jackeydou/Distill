// The one place the UI talks to `distill ui`. Tests swap in a fake with `setBridge`; a
// future desktop shell would swap in its own transport (spec D7).

export type Method = "GET" | "POST" | "PUT" | "DELETE";

export interface Bridge {
  request<T>(method: Method, path: string, body?: unknown): Promise<T>;
}

/** An API response that was not 2xx. `message` is the server's `error` text. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export const httpBridge: Bridge = {
  async request<T>(method: Method, path: string, body?: unknown): Promise<T> {
    const res = await fetch(`/api${path}`, {
      method,
      credentials: "same-origin",
      headers: body === undefined ? undefined : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) {
      throw new ApiError(res.status, await errorText(res));
    }
    if (res.status === 204) {
      return undefined as T;
    }
    return (await res.json()) as T;
  },
};

async function errorText(res: Response): Promise<string> {
  const text = await res.text();
  try {
    const parsed = JSON.parse(text) as { error?: unknown };
    if (typeof parsed.error === "string") {
      return parsed.error;
    }
  } catch {
    // Not JSON: a proxy or the static fallback answered. Show the raw text below.
  }
  return text || `HTTP ${res.status} for ${res.url}`;
}

let current: Bridge = httpBridge;

export function bridge(): Bridge {
  return current;
}

export function setBridge(next: Bridge): void {
  current = next;
}
