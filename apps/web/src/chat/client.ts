import { drainFrames } from "./sse";
import type { Answer, ChatSummary, ChatTurn, Me, Memory, Progress, Upload } from "./types";

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
  // A proxy in front of the engine can answer with its own HTML page.
  if (/^\s*<(!doctype|html)/i.test(message)) message = `Sparky is not reachable right now (${res.status}).`;
  return new EngineError(res.status, message || res.statusText);
};

/** Calls the engine with the session cookie and reads the JSON answer, throwing on a refusal. */
export const call = async <T>(method: string, path: string, body?: unknown): Promise<T> =>
  json<T>(
    await fetch(`${engineUrl()}${path}`, {
      method,
      credentials: "include",
      headers: body === undefined ? undefined : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  );

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
  files: Upload[] = [],
): Promise<Answer> => {
  const body = { user_id: "", message, conversation_id: conversationId, continue_channel: false, files };
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

/** What Sparky remembers about the student, in recall order. */
export const memories = async (): Promise<Memory[]> =>
  (await json<{ memories: Memory[] }>(await post("/memories/list", { user_id: "" }))).memories;

/** Forgets one memory by its number, or every memory when number is null. */
export const forget = async (number: number | null): Promise<number> =>
  (await json<{ removed: number }>(await post("/memories/forget", { user_id: "", number }))).removed;

/** The student's open web chats, newest first. */
export const chats = (): Promise<ChatSummary[]> => call<ChatSummary[]>("GET", "/web/conversations");

/** The questions and answers of one chat. */
export const chatTurns = (id: string): Promise<ChatTurn[]> => call<ChatTurn[]>("GET", `/web/conversations/${encodeURIComponent(id)}`);

/** Deletes one chat and its messages. */
export const deleteChat = async (id: string) => {
  await call<{ deleted: boolean }>("DELETE", `/web/conversations/${encodeURIComponent(id)}`);
};

/** Whether the student has linked a Canvas calendar feed. */
export const canvasStatus = async (): Promise<boolean> => (await call<{ linked: boolean }>("GET", "/canvas/feed")).linked;

/** Saves the Canvas calendar feed link. */
export const saveCanvas = async (link: string) => {
  await call<{ saved: boolean }>("POST", "/canvas/feed", { link });
};

/** Removes the Canvas calendar feed link. */
export const removeCanvas = async () => {
  await call<{ removed: boolean }>("POST", "/canvas/feed/remove", {});
};
