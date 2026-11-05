import type { Change, TraceRecord, TraceSummary } from "./client";

/** A time as the browser's local date and time, or the text as given when it is not a time. */
export const when = (iso: string) => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
};

/** A duration in milliseconds, short. */
export const ms = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)} s` : `${n} ms`);

/** One admin change in words. */
export const describe = (c: Change) => {
  if (c.kind === "tool") return `${c.key} ${c.value === true ? "on" : "off"}`;
  if (c.value === null) return "Instructions reset to the configured ones";
  return `Instructions: ${String(c.value).slice(0, 80)}`;
};

/** An event kind in words: tool_call becomes Tool call. */
export const kindName = (kind: string) => {
  const words = kind.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
};

/** Up to three short fields of a trace event, as a one-line summary. */
export const summary = (r: TraceRecord) =>
  Object.entries(r)
    .filter(([k, v]) => k !== "at" && k !== "kind" && (typeof v === "string" || typeof v === "number" || typeof v === "boolean"))
    .slice(0, 3)
    .map(([k, v]) => `${k.replace(/_/g, " ")}: ${String(v).slice(0, 60)}`)
    .join(", ");

/** The overview figures for the newest traces. */
export const figures = (rows: TraceSummary[]) => {
  const done = rows.filter((r) => r.status !== null);
  const times = done.map((r) => r.duration_ms).sort((a, b) => a - b);
  const median = times.length ? times[Math.floor((times.length - 1) / 2)] : null;
  const answered = done.filter((r) => r.status === "answered").length;
  return {
    requests: rows.length,
    median,
    tokens: rows.reduce((sum, r) => sum + r.tokens, 0),
    answeredShare: done.length ? Math.round((answered / done.length) * 100) : null,
    users: new Set(rows.map((r) => r.user_id)).size,
  };
};

/** Badge tones. */
export type Tone = "good" | "warn" | "bad" | "plain";

const STATUS: Record<string, [Tone, string]> = {
  answered: ["good", "Answered"],
  awaiting_confirmation: ["plain", "Waiting"],
  cancelled: ["plain", "Cancelled"],
  step_limit: ["warn", "Step limit"],
  stalled: ["warn", "Stalled"],
  deadline: ["warn", "Deadline"],
  blocked: ["bad", "Blocked"],
  error: ["bad", "Error"],
};

/** The tone and label of how a request ended; null is a request still running. */
export const status = (s: string | null): [Tone, string] => (s ? (STATUS[s] ?? ["plain", s]) : ["plain", "Running"]);

/** Button classes: the strong action, the quiet action, and a danger action. */
export const button = {
  primary: "inline-flex h-9 items-center gap-2 rounded-lg bg-fg px-3.5 text-sm font-medium text-canvas hover:opacity-90 disabled:opacity-40",
  secondary: "inline-flex h-9 items-center gap-2 rounded-lg border border-edge bg-raised px-3.5 text-sm hover:bg-sunk disabled:opacity-40",
  danger: "inline-flex h-8 items-center gap-2 rounded-lg border border-edge px-3 text-sm text-bad hover:border-bad/40 hover:bg-bad/10 disabled:opacity-40",
};
