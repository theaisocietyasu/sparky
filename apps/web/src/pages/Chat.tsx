import { FileText, RotateCcw } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import ChatFrame from "../app/ChatFrame";
import { useMe } from "../app/useMe";
import { ask, chatTurns, confirm, EngineError } from "../chat/client";
import { addFiles, toUpload } from "../chat/files";
import { applyProgress, type Line } from "../chat/lines";
import type { Answer } from "../chat/types";
import Composer from "../components/chat/Composer";
import Reply, { AssistantRow } from "../components/chat/Reply";
import Running from "../components/chat/Running";
import Sparky from "../components/sparky/Sparky";

/** One exchange. reveal is where the answer starts typing out, set only for an answer that arrived live. */
type Turn = { id: number; question: string; files?: string[]; answer: Answer | null; error: string | null; reveal?: number };

type Live = { lines: Line[]; draft: string; next: number };

const IDLE: Live = { lines: [], draft: "", next: 0 };

const SUGGESTIONS = [
  "What is due this week?",
  "When is Hayden Library open today?",
  "Find AI clubs I could join",
  "What do you remember about me?",
];

/** A greeting for the hour of the day. */
const greeting = (hour: number) => (hour < 5 ? "Up late" : hour < 12 ? "Good morning" : hour < 17 ? "Good afternoon" : "Good evening");

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

/** Pairs stored messages into exchanges. An answer with no question before it is dropped. */
const exchanges = (id: string, stored: { role: string; text: string }[]): Turn[] => {
  const out: Turn[] = [];
  for (const m of stored) {
    if (m.role === "user") out.push({ id: out.length + 1, question: m.text, answer: null, error: null });
    else {
      const last = out[out.length - 1];
      if (last && !last.answer) last.answer = { request_id: "", conversation_id: id, text: m.text, status: "answered" };
    }
  }
  return out;
};

/** /chat and /chat/:id: the web chat for a signed-in student, new or reopened. */
const Chat = () => {
  const { me } = useMe();
  const { id: opened } = useParams();
  const navigate = useNavigate();
  const [chats, setChats] = useState(0);
  const [shown, setShown] = useState<string | null>(null);
  const still = useReducedMotion() ?? false;
  const [turns, setTurns] = useState<Turn[]>([]);
  const [live, setLive] = useState<Live>(IDLE);
  const [text, setText] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const [fileError, setFileError] = useState<string | null>(null);
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

  // Loads a reopened chat; a chat this page just started is already on screen.
  useEffect(() => {
    const id = opened ?? null;
    if (id === conversation.current) return;
    abort.current?.abort();
    conversation.current = id;
    setTurns([]);
    if (!id) return;
    let live = true;
    chatTurns(id)
      .then((stored) => {
        if (!live) return;
        const loaded = exchanges(id, stored);
        counter.current = loaded.length;
        pinned.current = true;
        setTurns(loaded);
      })
      .catch(() => live && navigate("/chat", { replace: true }))
      .finally(() => live && setShown(id));
    return () => {
      live = false;
    };
  }, [opened, navigate]);

  const onShown = useCallback((shown: string) => {
    drafted.current = shown;
  }, []);

  const settle = (id: number, answer: Answer | null, error: string | null, reveal?: number) => {
    if (answer && answer.conversation_id !== conversation.current) {
      conversation.current = answer.conversation_id;
      navigate(`/chat/${answer.conversation_id}`, { replace: true });
      setChats((v) => v + 1);
    }
    setTurns((all) => all.map((t) => (t.id === id ? { ...t, answer, error, reveal } : t)));
  };

  const send = async (raw: string, attached: File[] = []) => {
    const question = raw.trim();
    if ((!question && attached.length === 0) || busy) return;
    const id = ++counter.current;
    pinned.current = true;
    drafted.current = "";
    setTurns((all) => [...all, { id, question, files: attached.map((f) => f.name), answer: null, error: null }]);
    setLiveId(id);
    setText("");
    setFiles([]);
    setFileError(null);
    setBusy(true);
    setLive(IDLE);
    const controller = new AbortController();
    abort.current = controller;
    try {
      const uploads = await Promise.all(attached.map(toUpload));
      const answer = await ask(question, conversation.current, (p) => setLive((s) => applyProgress(s, p)), controller.signal, uploads);
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

  const startOver = () => {
    abort.current?.abort();
    conversation.current = null;
    setTurns([]);
    navigate("/chat");
  };

  const loading = Boolean(opened) && opened !== shown && turns.length === 0;
  const empty = turns.length === 0 && !loading;
  const enter = still ? false : { opacity: 0, y: 8 };
  const composer = (
    <motion.div layoutId="composer" layout={!still} transition={{ type: "spring", stiffness: 380, damping: 36 }}>
      <Composer
        value={text}
        onChange={setText}
        onSend={() => send(text, files)}
        onStop={() => abort.current?.abort()}
        busy={busy}
        autoFocus
        files={files}
        onFiles={(more) => {
          const next = addFiles(files, more);
          setFiles(next.files);
          setFileError(next.error);
        }}
        onRemoveFile={(i) => {
          setFiles((all) => all.filter((_, j) => j !== i));
          setFileError(null);
        }}
      />
      {fileError && (
        <p role="alert" className="mt-2 px-4 text-sm text-bad">
          {fileError}
        </p>
      )}
    </motion.div>
  );

  return (
    <ChatFrame onNewChat={startOver} chats={chats}>
      {empty ? (
        <main className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-4 pb-16">
          <motion.div initial={enter} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.35, ease: "easeOut" }} className="w-full max-w-2xl">
            <motion.div
              initial={still ? false : { opacity: 0, scale: 0.9, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              transition={{ type: "spring", stiffness: 160, damping: 18, delay: 0.05 }}
              className="mx-auto w-fit"
            >
              <Sparky watch className="h-28 w-auto drop-shadow-[0_12px_24px_rgb(120_28_53/0.25)]" />
            </motion.div>
            <h1 className="mt-6 text-center text-2xl font-semibold tracking-tight sm:text-3xl">
              {greeting(new Date().getHours())}, {me.name.split(" ")[0]}
            </h1>
            <p className="mt-2 text-center text-[15px] text-dim">What is on your plate today?</p>
            <div className="mt-8">{composer}</div>
            <ul aria-label="Suggestions" className="mt-4 flex flex-wrap justify-center gap-2">
              {SUGGESTIONS.map((s, i) => (
                <motion.li
                  key={s}
                  initial={still ? false : { opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.3, ease: "easeOut", delay: 0.25 + i * 0.05 }}
                >
                  <button
                    type="button"
                    onClick={() => send(s)}
                    className="glass h-9 rounded-full px-3.5 text-sm text-dim transition-colors hover:text-fg"
                  >
                    {s}
                  </button>
                </motion.li>
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
                  <div className="flex max-w-[85%] flex-col items-end gap-2 self-end">
                    {t.files && t.files.length > 0 && (
                      <ul aria-label="Files sent" className="flex flex-wrap justify-end gap-2">
                        {t.files.map((name, i) => (
                          <li key={`${name}-${i}`} className="glass flex h-8 max-w-full items-center gap-1.5 rounded-lg px-2.5 text-sm">
                            <FileText className="size-4 shrink-0 text-dim" />
                            <span className="truncate">{name}</span>
                          </li>
                        ))}
                      </ul>
                    )}
                    {t.question && <p className="whitespace-pre-wrap rounded-3xl bg-shu px-4 py-2.5 text-[15px] leading-6 text-white">{t.question}</p>}
                  </div>
                  {t.answer && (
                    <Reply
                      answer={t.answer}
                      busy={busy}
                      revealFrom={t.reveal}
                      onConfirm={(approve) => answerHeld(t, approve)}
                      onChoose={t.id === turns[turns.length - 1]?.id ? (choice) => send(choice) : undefined}
                    />
                  )}
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
