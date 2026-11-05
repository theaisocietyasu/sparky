import { RotateCcw } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import ChatFrame from "../app/ChatFrame";
import { useMe } from "../app/useMe";
import { ask, confirm, EngineError, reset } from "../chat/client";
import { applyProgress, type Line } from "../chat/lines";
import type { Answer } from "../chat/types";
import Composer from "../components/chat/Composer";
import Reply, { AssistantRow } from "../components/chat/Reply";
import Running from "../components/chat/Running";

/** One exchange. reveal is where the answer starts typing out, set only for an answer that arrived live. */
type Turn = { id: number; question: string; answer: Answer | null; error: string | null; reveal?: number };

type Live = { lines: Line[]; draft: string; next: number };

const IDLE: Live = { lines: [], draft: "", next: 0 };

const SUGGESTIONS = [
  "What is due this week?",
  "When is Hayden Library open today?",
  "Find AI clubs I could join",
  "What do you remember about me?",
];

const HELD = new Set(["Approve", "Cancel"]);

const message = (e: unknown) => {
  if (e instanceof DOMException && e.name === "AbortError") return "Stopped.";
  if (e instanceof EngineError) {
    if (e.status === 401) return "Your session ended. Sign in again.";
    if (e.status === 429) return "Too many messages. Wait a minute.";
    if (e.status === 503) return "Sparky is busy. Try again shortly.";
    return e.message;
  }
  return "Sparky is not reachable right now.";
};

/** How much of the answer the draft already showed, so the answer types on from there. */
const shared = (a: string, b: string) => {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  return i;
};

/** /chat: the web chat for a signed-in student. */
const Chat = () => {
  const { me } = useMe();
  const still = useReducedMotion() ?? false;
  const [turns, setTurns] = useState<Turn[]>([]);
  const [live, setLive] = useState<Live>(IDLE);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [liveId, setLiveId] = useState(0);
  const conversation = useRef<string | null>(null);
  const counter = useRef(0);
  const abort = useRef<AbortController | null>(null);
  const drafted = useRef("");
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  // Follows new text only while the reader is at the bottom, so scrolling up to read is never pulled back.
  const follow = () => {
    const el = scroller.current;
    if (el) pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  };
  useEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  });

  const onShown = useCallback((shown: string) => {
    drafted.current = shown;
  }, []);

  const settle = (id: number, answer: Answer | null, error: string | null, reveal?: number) => {
    if (answer) conversation.current = answer.conversation_id;
    setTurns((all) => all.map((t) => (t.id === id ? { ...t, answer, error, reveal } : t)));
  };

  const send = async (raw: string) => {
    const question = raw.trim();
    if (!question || busy) return;
    const id = ++counter.current;
    pinned.current = true;
    drafted.current = "";
    setTurns((all) => [...all, { id, question, answer: null, error: null }]);
    setLiveId(id);
    setText("");
    setBusy(true);
    setLive(IDLE);
    const controller = new AbortController();
    abort.current = controller;
    try {
      const answer = await ask(question, conversation.current, (p) => setLive((s) => applyProgress(s, p)), controller.signal);
      settle(id, answer, null, shared(drafted.current, answer.text));
    } catch (err) {
      settle(id, null, message(err));
    } finally {
      abort.current = null;
      setBusy(false);
      setLive(IDLE);
    }
  };

  const answerHeld = async (turn: Turn, approve: boolean) => {
    const held = turn.answer?.confirmation;
    if (!held || !turn.answer) return;
    setBusy(true);
    const id = ++counter.current;
    setLiveId(id);
    setTurns((all) => [
      ...all.map((t) => (t.id === turn.id && t.answer ? { ...t, answer: { ...t.answer, confirmation: null } } : t)),
      { id, question: approve ? "Approve" : "Cancel", answer: null, error: null },
    ]);
    try {
      settle(id, await confirm(held.token, approve, turn.answer.conversation_id), null, 0);
    } catch (err) {
      settle(id, null, message(err));
    } finally {
      setBusy(false);
    }
  };

  const startOver = async () => {
    abort.current?.abort();
    try {
      await reset();
    } catch {
      // A failed reset leaves the old conversation open; the next turn still runs.
    }
    conversation.current = null;
    setTurns([]);
  };

  const empty = turns.length === 0;
  const enter = still ? false : { opacity: 0, y: 8 };
  const composer = (
    <motion.div layoutId="composer" layout={!still} transition={{ type: "spring", stiffness: 380, damping: 36 }}>
      <Composer value={text} onChange={setText} onSend={() => send(text)} onStop={() => abort.current?.abort()} busy={busy} autoFocus />
    </motion.div>
  );

  return (
    <ChatFrame onNewChat={startOver}>
      {empty ? (
        <main className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-4 pb-16">
          <motion.div initial={enter} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.35, ease: "easeOut" }} className="w-full max-w-2xl">
            <img src="/brand/sparkyai-logo.png" alt="" width={717} height={779} className="mx-auto h-12 w-auto" />
            <h1 className="mt-5 text-center text-2xl font-semibold tracking-tight sm:text-3xl">
              What can I help with, {me.name.split(" ")[0]}?
            </h1>
            <div className="mt-8">{composer}</div>
            <ul aria-label="Suggestions" className="mt-4 flex flex-wrap justify-center gap-2">
              {SUGGESTIONS.map((s) => (
                <li key={s}>
                  <button
                    type="button"
                    onClick={() => send(s)}
                    className="h-9 rounded-full border border-edge px-3.5 text-sm text-dim transition-colors hover:bg-sunk hover:text-fg"
                  >
                    {s}
                  </button>
                </li>
              ))}
            </ul>
          </motion.div>
        </main>
      ) : (
        <main className="flex min-h-0 flex-1 flex-col">
          <div ref={scroller} onScroll={follow} className="flex-1 overflow-y-auto [scrollbar-gutter:stable]">
            <ol aria-label="Conversation" className="mx-auto flex w-full max-w-3xl flex-col gap-8 px-4 py-8">
              {turns.map((t) => (
                <motion.li key={t.id} initial={enter} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.25, ease: "easeOut" }} className="flex flex-col gap-6">
                  <p className="max-w-[85%] self-end whitespace-pre-wrap rounded-3xl bg-sunk px-4 py-2.5 text-[15px] leading-6">{t.question}</p>
                  {t.answer && <Reply answer={t.answer} busy={busy} revealFrom={t.reveal} onConfirm={(approve) => answerHeld(t, approve)} />}
                  {t.error && (
                    <AssistantRow>
                      <div className="flex flex-wrap items-center gap-3">
                        <p role="alert" className="text-sm text-bad">
                          {t.error}
                        </p>
                        {!busy && !HELD.has(t.question) && (
                          <button
                            type="button"
                            onClick={() => send(t.question)}
                            className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-dim hover:bg-sunk hover:text-fg"
                          >
                            <RotateCcw className="size-3.5" />
                            Try again
                          </button>
                        )}
                      </div>
                    </AssistantRow>
                  )}
                  {!t.answer && !t.error && busy && t.id === liveId && <Running lines={live.lines} draft={live.draft} onShown={onShown} />}
                </motion.li>
              ))}
            </ol>
          </div>
          <div className="mx-auto w-full max-w-3xl px-4 pb-4">
            {composer}
            <p className="mt-2 text-center text-xs text-dim">Sparky can be wrong. Check the sources it links.</p>
          </div>
        </main>
      )}
    </ChatFrame>
  );
};

export default Chat;
