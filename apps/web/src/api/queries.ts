import { QueryClient, queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { ApiError } from "./bridge";
import { api } from "./client";
import type { NotesQuery } from "./generated/NotesQuery";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Freshness comes from the server's change events, not from polling.
      staleTime: Number.POSITIVE_INFINITY,
      retry: (count, error) => !(error instanceof ApiError && error.status < 500) && count < 2,
    },
  },
});

export const q = {
  session: () => queryOptions({ queryKey: ["session"], queryFn: api.session }),
  notes: (query: NotesQuery) =>
    queryOptions({ queryKey: ["notes", query], queryFn: () => api.notes(query) }),
  note: (id: string) => queryOptions({ queryKey: ["note", id], queryFn: () => api.note(id) }),
  topics: () => queryOptions({ queryKey: ["topics"], queryFn: api.topics }),
  topic: (id: string) => queryOptions({ queryKey: ["topic", id], queryFn: () => api.topic(id) }),
  tags: () => queryOptions({ queryKey: ["tags"], queryFn: api.tags }),
  stats: () => queryOptions({ queryKey: ["stats"], queryFn: api.stats }),
  problems: () => queryOptions({ queryKey: ["problems"], queryFn: api.problems }),
};

/** Refetches everything on screen whenever the server reports a vault change. */
export function useVaultEvents(enabled: boolean): void {
  const client = useQueryClient();
  useEffect(() => {
    if (!enabled) {
      return;
    }
    const events = new EventSource("/api/events");
    const refresh = () => {
      void client.invalidateQueries({ predicate: (query) => query.queryKey[0] !== "session" });
    };
    events.addEventListener("changed", refresh);
    // After a reconnect (server restarted), anything may have changed.
    events.addEventListener("open", refresh);
    return () => events.close();
  }, [client, enabled]);
}

/** A mutation that refreshes every query when it succeeds. */
export function useEdit<A, R>(fn: (args: A) => Promise<R>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => client.invalidateQueries(),
  });
}
