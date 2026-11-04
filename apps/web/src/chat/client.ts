import { drainFrames } from "./sse";
import type { Answer, Me, Memory, Progress } from "./types";

/** The engine origin the page talks to, or empty when the build has none. */
export const engineUrl = () => (import.meta.env.VITE_ENGINE_URL ?? "").replace(/\/$/, "");

/** A refused or failed engine call. */
export class EngineError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

const post = (path: string, body: unknown, signal?: AbortSignal) =>
  fetch(`${engineUrl()}${path}`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
    signal,
  });

const failure = async (res: Response) => {
  const text = await res.text().catch(() => "");
  let message = text;
  try {
    message = (JSON.parse(text) as { error?: string }).error ?? text;
  } catch {
    // The body is plain text.
  }
  return new EngineError(res.status, message || res.statusText);
};

const json = async <T>(res: Response): Promise<T> => {
  if (!res.ok) throw await failure(res);
  return (await res.json()) as T;
};

/** Where the sign-in starts. */
export const loginUrl = () => `${engineUrl()}/web/login`;

/** The signed-in student, or null when there is no session. */
export const me = async (): Promise<Me | null> => {
  const res = await fetch(`${engineUrl()}/web/me`, { credentials: "include" });
  if (res.status === 401) return null;
  return json<Me>(res);
};

/** Ends the session. */
export const logout = async () => {
  const res = await post("/web/logout", {});
  if (!res.ok && res.status !== 401) throw await failure(res);
};

/** Runs one turn, calling onProgress for each progress event, and returns the answer. */
export const ask = async (
  message: string,
  conversationId: string | null,
  onProgress: (p: Progress) => void,
  signal?: AbortSignal,
): Promise<Answer> => {
  const body = { user_id: "", message, conversation_id: conversationId, continue_channel: conversationId === null };
  const res = await post("/chat/stream", body, signal);
  if (!res.ok || !res.body) throw await failure(res);
  const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (value) buffer += value;
    const { frames, rest } = drainFrames(done ? `${buffer}\n\n` : buffer);
    buffer = rest;
    for (const frame of frames) {
      if (frame.name === "progress") onProgress(JSON.parse(frame.data) as Progress);
      else if (frame.name === "answer") return JSON.parse(frame.data) as Answer;
      else if (frame.name === "error") {
        const e = JSON.parse(frame.data) as { error?: string; status?: number };
        throw new EngineError(e.status ?? 502, e.error ?? "the turn failed");
      }
    }
    if (done) throw new EngineError(502, "the stream ended without an answer");
  }
};

/** Approves or cancels an action the engine holds. */
export const confirm = async (token: string, approve: boolean, conversationId: string): Promise<Answer> =>
  json<Answer>(await post("/confirm", { token, approve, user_id: "", conversation_id: conversationId }));

/** Ends the open web conversation so the next message starts a new one. */
export const reset = async () => {
  const res = await post("/conversation/reset", { user_id: "", channel_id: "web" });
  if (!res.ok) throw await failure(res);
};

/** What Sparky remembers about the student, in recall order. */
export const memories = async (): Promise<Memory[]> =>
  (await json<{ memories: Memory[] }>(await post("/memories/list", { user_id: "" }))).memories;

/** Forgets one memory by its number, or every memory when number is null. */
export const forget = async (number: number | null): Promise<number> =>
  (await json<{ removed: number }>(await post("/memories/forget", { user_id: "", number }))).removed;
