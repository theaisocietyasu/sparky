import { type FormEvent, useRef, useState } from "react";
import { ask, confirm, EngineError, reset } from "../chat/client";
import { applyProgress, type Line } from "../chat/lines";
import type { Answer } from "../chat/types";
import Gated from "../components/chat/Gated";
import Reply from "../components/chat/Reply";

type Turn = { id: number; question: string; answer: Answer | null; error: string | null };

const message = (e: unknown) => {
  if (e instanceof EngineError) {
    if (e.status === 401) return "Your session ended. Sign in again.";
    if (e.status === 429) return "Too many messages. Wait a minute.";
    if (e.status === 503) return "Sparky is busy. Try again shortly.";
    return e.message;
  }
  return "Sparky is not reachable right now.";
};

/** The conversation: past turns, the live progress of the running one, and the message box. */
const Conversation = () => {
  const [turns, setTurns] = useState<Turn[]>([]);
  const [progress, setProgress] = useState<{ lines: Line[]; draft: string; next: number }>({
    lines: [],
    draft: "",
    next: 0,
  });
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const conversation = useRef<string | null>(null);
  const counter = useRef(0);

  const settle = (id: number, answer: Answer | null, error: string | null) => {
    if (answer) conversation.current = answer.conversation_id;
    setTurns((all) => all.map((t) => (t.id === id ? { ...t, answer, error } : t)));
  };

  const send = async (e: FormEvent) => {
    e.preventDefault();
    const question = text.trim();
    if (!question || busy) return;
    const id = ++counter.current;
    setTurns((all) => [...all, { id, question, answer: null, error: null }]);
    setText("");
    setBusy(true);
    setProgress({ lines: [], draft: "", next: 0 });
    try {
      const answer = await ask(question, conversation.current, (p) => setProgress((s) => applyProgress(s, p)));
      settle(id, answer, null);
    } catch (err) {
      settle(id, null, message(err));
    } finally {
      setBusy(false);
      setProgress({ lines: [], draft: "", next: 0 });
    }
  };

  const answerHeld = async (turn: Turn, approve: boolean) => {
    const held = turn.answer?.confirmation;
    if (!held || !turn.answer) return;
    setBusy(true);
    const id = ++counter.current;
    setTurns((all) => [
      ...all.map((t) => (t.id === turn.id && t.answer ? { ...t, answer: { ...t.answer, confirmation: null } } : t)),
      { id, question: approve ? "Approve" : "Cancel", answer: null, error: null },
    ]);
    try {
      settle(id, await confirm(held.token, approve, turn.answer.conversation_id), null);
    } catch (err) {
      settle(id, null, message(err));
    } finally {
      setBusy(false);
    }
  };

  const startOver = async () => {
    try {
      await reset();
    } catch {
      // A failed reset leaves the old conversation open; the next turn still runs.
    }
    conversation.current = null;
    setTurns([]);
  };

  return (
    <>
      <div className="flex items-center justify-between">
        <h1 className="font-serif text-2xl font-bold">Chat</h1>
        <button type="button" onClick={startOver} disabled={busy} className="text-sm text-ink-soft hover:text-ink disabled:opacity-50">
          New conversation
        </button>
      </div>
      <ol aria-label="Conversation" className="mt-4 flex flex-1 flex-col gap-4">
        {turns.map((t) => (
          <li key={t.id} className="flex flex-col gap-2">
            <p className="self-end rounded-md bg-ink px-3 py-2 text-sm text-paper">{t.question}</p>
            {t.answer && <Reply answer={t.answer} busy={busy} onConfirm={(approve) => answerHeld(t, approve)} />}
            {t.error && (
              <p role="alert" className="text-sm text-shu">
                {t.error}
              </p>
            )}
          </li>
        ))}
        {busy && (
          <li aria-live="polite" aria-label="Progress" className="flex flex-col gap-1 text-sm text-ink-soft">
            {progress.lines.map((l) => (
              <p key={l.key}>{l.text}</p>
            ))}
            {progress.draft && <p className="whitespace-pre-wrap text-ink">{progress.draft}</p>}
            {progress.lines.length === 0 && !progress.draft && <p>Thinking...</p>}
          </li>
        )}
      </ol>
      <form onSubmit={send} className="sticky bottom-0 mt-6 flex gap-2 bg-paper py-3">
        <label htmlFor="message" className="sr-only">
          Message
        </label>
        <input
          id="message"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Ask Sparky"
          autoComplete="off"
          className="h-11 flex-1 rounded-full border border-ink/20 bg-white px-4 text-sm outline-none focus:border-shu"
        />
        <button
          type="submit"
          disabled={busy || !text.trim()}
          className="h-11 rounded-full bg-shu px-5 text-sm font-semibold text-paper disabled:opacity-50"
        >
          Send
        </button>
      </form>
    </>
  );
};

/** /chat: the web chat for a signed-in student. */
const Chat = () => <Gated>{() => <Conversation />}</Gated>;

export default Chat;
