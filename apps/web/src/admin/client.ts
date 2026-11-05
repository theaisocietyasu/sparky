import { call } from "../chat/client";

/** One tool and whether it is offered. */
export type ToolState = { name: string; description: string; on: boolean; private: boolean };

/** The instructions in force and the configured ones. */
export type Instructions = { text: string; default_text: string; changed: boolean; max_chars: number };

/** One recorded admin change. */
export type Change = {
  id: number;
  kind: "tool" | "instructions";
  key: string;
  value: unknown;
  actor: string;
  created_at: string;
};

/** An action waiting on a student. */
export type Held = { token: string; user_id: string; tool: string; created_at: string; expires_at: string };

/** The settings that bound load. */
export type Limits = Record<string, number>;

export const tools = () => call<ToolState[]>("GET", "/admin/tools");
export const switchTool = (name: string, on: boolean) =>
  call<Change>("POST", `/admin/tools/${encodeURIComponent(name)}`, { on });
export const instructions = () => call<Instructions>("GET", "/admin/instructions");
export const setInstructions = (text: string | null) => call<Change>("PUT", "/admin/instructions", { text });
export const history = () => call<Change[]>("GET", "/admin/history");
export const rollback = (id: number) => call<Change>("POST", `/admin/rollback/${id}`);
export const limits = () => call<Limits>("GET", "/admin/limits");

/** What the guardrail refuses or hides, and what the policy allows. */
export type Guardrails = {
  enabled: boolean;
  denied_phrases: string[];
  protected_terms: string[];
  redaction: string;
  max_answer_chars: number;
  replacement: string;
  write_roles: string[];
  allow_authenticated_reads: boolean;
  confirm_from: string;
};

export const guardrails = () => call<Guardrails>("GET", "/admin/guardrails");
export const approvals = () => call<Held[]>("GET", "/admin/approvals");
export const deny = (token: string) => call<{ denied: boolean }>("POST", `/admin/approvals/${encodeURIComponent(token)}/deny`);

/** One request in the trace list. */
export type TraceSummary = {
  request_id: string;
  conversation_id: string;
  at: string;
  user_id: string;
  input: string;
  status: string | null;
  steps: number;
  tokens: number;
  duration_ms: number;
  trace_id?: string | null;
  phoenix_url?: string | null;
  guardrail_blocks?: number;
  guardrail_redactions?: number;
};

/** One recorded trace event. */
export type TraceRecord = { at: string; kind: string } & Record<string, unknown>;

/** One live query source Platform publishes. */
export type Source = { key: string; indexed: boolean; params: { name: string; required: boolean }[] };

export const traces = () => call<TraceSummary[]>("GET", "/admin/traces");
export const trace = (id: string) => call<TraceRecord[]>("GET", `/admin/traces/${encodeURIComponent(id)}`);
export const sources = () => call<Source[]>("GET", "/admin/sources");
