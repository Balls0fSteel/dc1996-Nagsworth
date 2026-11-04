// In-page notifications for the main window, rendered by `ui/Toasts.svelte`.
//   notify.ok("Saved");  notify.err(e);

export type NoticeKind = "ok" | "err";

export interface Notice {
  id: number;
  kind: NoticeKind;
  text: string;
}

const TTL_MS = 3000;

let items = $state<Notice[]>([]);
let next = 1;

function push(kind: NoticeKind, text: string): number {
  const id = next++;
  items = [...items, { id, kind, text }].slice(-5);
  setTimeout(() => dismiss(id), kind === "err" ? TTL_MS * 2 : TTL_MS);
  return id;
}

function dismiss(id: number) {
  items = items.filter((n) => n.id !== id);
}

/** Turns anything thrown into a message. */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export const notify = {
  ok: (text: string) => push("ok", text),
  /** Accepts a string or anything caught. */
  err: (e: unknown) => push("err", errorText(e)),
  dismiss,
  get list(): readonly Notice[] {
    return items;
  },
};
