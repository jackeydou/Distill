import { bridge } from "./bridge";
import type { AnnotationInput } from "./generated/AnnotationInput";
import type { ConflictChoice } from "./generated/ConflictChoice";
import type { Merged } from "./generated/Merged";
import type { NoteDetail } from "./generated/NoteDetail";
import type { NoteHit } from "./generated/NoteHit";
import type { NotesQuery } from "./generated/NotesQuery";
import type { Problems } from "./generated/Problems";
import type { Session } from "./generated/Session";
import type { Stats } from "./generated/Stats";
import type { TagCount } from "./generated/TagCount";
import type { TopicCount } from "./generated/TopicCount";
import type { TopicDetail } from "./generated/TopicDetail";
import type { TopicMerge } from "./generated/TopicMerge";
import type { TopicRename } from "./generated/TopicRename";

const enc = encodeURIComponent;

export function queryString(params: object): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      search.set(key, String(value));
    }
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}

export const api = {
  session: () => bridge().request<Session>("GET", "/session"),
  notes: (q: NotesQuery) => bridge().request<NoteHit[]>("GET", `/notes${queryString(q)}`),
  note: (id: string) => bridge().request<NoteDetail>("GET", `/notes/${enc(id)}`),
  addAnnotation: (noteId: string, body: AnnotationInput) =>
    bridge().request<NoteDetail>("POST", `/notes/${enc(noteId)}/annotations`, body),
  updateAnnotation: (id: string, body: AnnotationInput) =>
    bridge().request<void>("PUT", `/annotations/${enc(id)}`, body),
  deleteAnnotation: (id: string) => bridge().request<void>("DELETE", `/annotations/${enc(id)}`),
  topics: () => bridge().request<TopicCount[]>("GET", "/topics"),
  topic: (id: string) => bridge().request<TopicDetail>("GET", `/topics/${enc(id)}`),
  renameTopic: (id: string, body: TopicRename) =>
    bridge().request<void>("PUT", `/topics/${enc(id)}`, body),
  mergeTopic: (id: string, body: TopicMerge) =>
    bridge().request<Merged>("POST", `/topics/${enc(id)}/merge`, body),
  tags: () => bridge().request<TagCount[]>("GET", "/tags"),
  stats: () => bridge().request<Stats>("GET", "/stats"),
  problems: () => bridge().request<Problems>("GET", "/problems"),
  resolveConflict: (body: ConflictChoice) =>
    bridge().request<void>("POST", "/conflicts/resolve", body),
};
