import { ArrowLeft, BookOpen, CalendarClock, ChevronRight, FileText, MessageSquareText, RotateCcw, Sparkles, Users, type LucideIcon } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";
import ChatFrame from "../app/ChatFrame";
import { arrive, EASE, settled, spring } from "../app/motion";
import { useMe } from "../app/useMe";
import { ask, chats as listChats, chatTurns, confirm, deleteChat, EngineError, homeId, readNotices, type ThreadStart } from "../chat/client";
import { addFiles, toUpload } from "../chat/files";
import { useFollow } from "../chat/useFollow";
import { applyProgress, type Line } from "../chat/lines";
import type { Answer, ChatSummary, ChatTurn } from "../chat/types";
import ClearHome from "../components/chat/ClearHome";
import Composer from "../components/chat/Composer";
import Reply, { AssistantRow } from "../components/chat/Reply";
import Running from "../components/chat/Running";
import Sparky from "../components/sparky/Sparky";

/**
 * One exchange. A message Sparky sent on its own has no question. position is where a stored answer sits and
 * asked where the stored question sits, the places a thread names. reveal is where the answer starts typing out,
 * set only for an answer that arrived live.
 */
type Turn = {
  id: number;
  question: string;
  files?: string[];
  answer: Answer | null;
  error: string | null;
  reveal?: number;
  position?: number;
  asked?: number;
};

const EVERY_MS = 60_000;

type Live = { lines: Line[]; draft: string; next: number };

const IDLE: Live = { lines: [], draft: "", next: 0 };

const SUGGESTIONS: { text: string; icon: LucideIcon }[] = [
  { text: "What is due this week?", icon: CalendarClock },
  { text: "When is Hayden Library open today?", icon: BookOpen },
  { text: "Find AI clubs I could join", icon: Users },
  { text: "What do you remember about me?", icon: Sparkles },
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

/** Pairs stored messages into exchanges. An answer with no question before it stands alone. */
const exchanges = (id: string, stored: ChatTurn[], from = 0): Turn[] => {
  const out: Turn[] = [];
  for (const m of stored) {
    if (m.role === "user") {
      out.push({ id: from + out.length + 1, question: m.text, answer: null, error: null, asked: m.position });
      continue;
    }
    const answer: Answer = { request_id: "", conversation_id: id, text: m.text, status: "answered", notice: m.notice };
    const last = out[out.length - 1];
    if (last && !last.answer && !m.notice) Object.assign(last, { answer, position: m.position });
    else out.push({ id: from + out.length + 1, question: "", answer, error: null, position: m.position });
  }
  return out;
};

/** The newest stored position, or 0. */
const newest = (stored: ChatTurn[]) => stored.reduce((n, m) => Math.max(n, m.position ?? 0), 0);

/**
 * /chat is home: the one chat Sparky's own messages land in. /chat/:id is a thread or an older chat, and
 * /chat/new starts a thread off the home message the page was sent with.
 */
/** The link under a home question to the thread it was answered in. */
const MovedTo = ({ thread }: { thread?: ChatSummary }) =>
  thread ? (
    <Link
      to={`/chat/${thread.id}`}
      className="glass inline-flex h-8 max-w-full items-center gap-1.5 rounded-full px-3 text-xs text-dim transition-colors hover:text-fg"
    >
      <MessageSquareText className="size-3.5 shrink-0" />
      <span className="truncate">Thread: {thread.title}</span>
      <ChevronRight className="size-3.5 shrink-0" />
    </Link>
  ) : null;

const Chat = () => {
  const { me } = useMe();
  const { id: opened } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const [chats, setChats] = useState(0);
  const [seen, setSeen] = useState(0);
  const [home, setHome] = useState<string | null | undefined>(undefined);
  const [threads, setThreads] = useState<ChatSummary[]>([]);
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
  const latest = useRef(0);
  const busyNow = useRef(false);
  const abort = useRef<AbortController | null>(null);
  const drafted = useRef("");
  const { bind: scroll, pin, follow, jump } = useFollow(still);
  const landed = useRef(false);
  // Bumped when home is cleared, so the page loads home again under the same id.
  const [epoch, setEpoch] = useState(0);

  const start = opened === "new" ? ((location.state as ThreadStart | null) ?? null) : null;
  // undefined while the home id loads; null is a new conversation.
  const target = opened === "new" ? null : (opened ?? home);
  const atHome = Boolean(home) && target === home;

  useEffect(() => {
    let live = true;
    homeId()
      .then((id) => live && setHome(id))
      .catch(() => live && setHome(null));
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!home) return;
    let live = true;
    listChats()
      .then((all) => live && setThreads(all.filter((c) => c.parent?.conversation_id === home)))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [home, chats]);

  // Follows new text only while the reader is at the bottom, so scrolling up to read is never pulled back.
  // A chat that just loaded jumps to its end; text that arrives after glides into view.
  useEffect(() => {
    if (landed.current) {
      landed.current = false;
      jump();
    } else follow();
  });

  const markRead = useCallback(() => {
    readNotices()
      .then(() => setSeen((v) => v + 1))
      .catch(() => undefined);
  }, []);

  // Loads the chat the page is on; a chat this page just started is already on screen.
  useEffect(() => {
    if (target === undefined) return;
    if (target === conversation.current) return;
    abort.current?.abort();
    conversation.current = target;
    latest.current = 0;
    setTurns([]);
    if (!target) return;
    let live = true;
    chatTurns(target)
      .then((stored) => {
        if (!live) return;
        const loaded = exchanges(target, stored);
        counter.current = loaded.length;
        latest.current = newest(stored);
        pin();
        landed.current = true;
        setTurns(loaded);
        if (target === home) markRead();
      })
      .catch(() => live && navigate("/chat", { replace: true }))
      .finally(() => live && setShown(target));
    return () => {
      live = false;
    };
  }, [target, home, navigate, markRead, epoch, pin]);

  // While home is open, Sparky's new messages appear in it.
  useEffect(() => {
    if (!home || !atHome) return;
    const check = () => {
      if (document.visibilityState !== "visible" || busyNow.current) return;
      chatTurns(home)
        .then((stored) => {
          if (conversation.current !== home || busyNow.current) return;
          const fresh = stored.filter((m) => m.notice && (m.position ?? 0) > latest.current);
          latest.current = Math.max(latest.current, newest(stored));
          if (fresh.length === 0) return;
          const added = exchanges(home, fresh, counter.current);
          counter.current += added.length;
          setTurns((all) => [...all, ...added]);
          markRead();
        })
        .catch(() => undefined);
    };
    const timer = window.setInterval(check, EVERY_MS);
    return () => window.clearInterval(timer);
  }, [home, atHome, markRead]);

  /** Deletes home and its messages; the engine makes it again with a fresh welcome. */
  const clearHome = async () => {
    if (!home || busy) return;
    try {
      await deleteChat(home);
      await homeId();
    } catch {
      return;
    }
    conversation.current = null;
    setShown(null);
    setEpoch((v) => v + 1);
    setChats((v) => v + 1);
  };

  const onShown = useCallback((shown: string) => {
    drafted.current = shown;
  }, []);

  const settle = (id: number, answer: Answer | null, error: string | null, reveal?: number) => {
    if (answer && answer.conversation_id !== conversation.current) {
      // A new topic asked in home comes back in a thread of its own; Back returns to home.
      const fromHome = conversation.current === home;
      conversation.current = answer.conversation_id;
      navigate(`/chat/${answer.conversation_id}`, { replace: !fromHome });
      setChats((v) => v + 1);
    }
    setTurns((all) => all.map((t) => (t.id === id ? { ...t, answer, error, reveal } : t)));
  };

  const send = async (raw: string, attached: File[] = []) => {
    const question = raw.trim();
    if ((!question && attached.length === 0) || busy) return;
    const id = ++counter.current;
    pin();
    drafted.current = "";
    setTurns((all) => [...all, { id, question, files: attached.map((f) => f.name), answer: null, error: null }]);
    setLiveId(id);
    setText("");
    setFiles([]);
    setFileError(null);
    setBusy(true);
    busyNow.current = true;
    setLive(IDLE);
    const controller = new AbortController();
    abort.current = controller;
    try {
      const uploads = await Promise.all(attached.map(toUpload));
      const onProgress = (p: Parameters<typeof applyProgress>[1]) => setLive((s) => applyProgress(s, p));
      const answer = await ask(question, conversation.current, onProgress, controller.signal, uploads, start ?? undefined);
      settle(id, answer, null, shared(drafted.current, answer.text));
    } catch (err) {
      settle(id, null, message(err));
    } finally {
      abort.current = null;
      busyNow.current = false;
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

  /** The thread controls under one home message. */
  const threadsOf = (t: Turn) => {
    if (!atHome || !home || t.position === undefined || !t.answer) return undefined;
    const position = t.position;
    const quote = t.answer.text;
    const mine = threads.filter((c) => c.parent?.position === position);
    return {
      count: mine.length,
      onOpen: mine[0] ? () => navigate(`/chat/${mine[0].id}`) : undefined,
      onStart: () => navigate("/chat/new", { state: { parent: { conversation_id: home, position }, quote } satisfies ThreadStart }),
    };
  };

  /** The thread a home question was moved into, if any. */
  const threadOfQuestion = (t: Turn) =>
    atHome && t.asked !== undefined ? threads.find((c) => c.parent?.position === t.asked) : undefined;

  const waiting = target === undefined;
  const loading = waiting || (Boolean(target) && target !== shown && turns.length === 0);
  const empty = turns.length === 0 && !loading && !start;
  const thread = Boolean(home) && !atHome && !waiting;
  const enter = arrive(still);
  const composer = (
    <motion.div layoutId="composer" layout={!still} transition={spring.glide}>
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
    <ChatFrame
      chats={chats}
      seen={seen}
      tools={atHome && turns.length > 0 ? <ClearHome onClear={clearHome} disabled={busy} /> : undefined}
      crumb={
        thread ? (
          <Link to="/chat" aria-label="Back to home" className="inline-flex h-8 items-center gap-1.5 rounded-full px-2.5 text-sm text-dim transition-colors hover:bg-sunk/70 hover:text-fg">
            <ArrowLeft className="size-4" />
            <span className="hidden sm:inline">Home</span>
            <span aria-hidden className="text-edge">/</span>
            <span>Thread</span>
          </Link>
        ) : undefined
      }
    >
      {waiting ? (
        <main className="flex-1" />
      ) : empty ? (
        <main className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-4 pb-16 pt-20">
          <motion.div initial={enter} animate={settled} transition={spring.soft} className="w-full max-w-2xl">
            <motion.div
              layoutId="sparky-hero"
              initial={still ? false : { opacity: 0, scale: 0.9, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              transition={{ type: "spring", stiffness: 160, damping: 18, delay: 0.05 }}
              className="mx-auto w-fit"
            >
              <Sparky watch className="h-28 w-auto drop-shadow-[0_12px_24px_rgb(120_28_53/0.25)]" />
            </motion.div>
            <motion.h1
              initial={arrive(still, { y: 14 })}
              animate={settled}
              transition={{ ...spring.soft, delay: 0.08 }}
              className="mt-6 text-center text-3xl font-semibold tracking-tight sm:text-4xl"
            >
              {greeting(new Date().getHours())}, {me.name.split(" ")[0]}
            </motion.h1>
            <motion.p
              initial={arrive(still, { y: 10 })}
              animate={settled}
              transition={{ ...spring.soft, delay: 0.14 }}
              className="mt-2 text-center text-[15px] text-dim"
            >
              What is on your plate today?
            </motion.p>
            <div className="mt-8">{composer}</div>
            <ul aria-label="Suggestions" className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2">
              {SUGGESTIONS.map((s, i) => (
                <motion.li key={s.text} initial={arrive(still, { y: 12 })} animate={settled} transition={{ ...spring.soft, delay: 0.22 + i * 0.05 }}>
                  <motion.button
                    type="button"
                    onClick={() => send(s.text)}
                    whileHover={still ? undefined : { y: -2 }}
                    whileTap={still ? undefined : { scale: 0.97 }}
                    transition={spring.snap}
                    className="glass group flex h-12 w-full items-center gap-3 rounded-2xl px-4 text-left text-sm text-dim transition-colors hover:text-fg"
                  >
                    <s.icon className="size-4 shrink-0 text-accent transition-transform group-hover:scale-110" />
                    <span className="truncate">{s.text}</span>
                  </motion.button>
                </motion.li>
              ))}
            </ul>
          </motion.div>
        </main>
      ) : (
        <main className="relative flex min-h-0 flex-1 flex-col">
          <div {...scroll} className="fade-under-bar flex-1 overflow-y-auto [scrollbar-gutter:stable]">
            {start && (
              <blockquote aria-label="Thread of" className="mx-auto mt-28 w-full max-w-3xl px-4">
                <p className="glass line-clamp-4 rounded-2xl px-4 py-3 text-sm text-dim">{start.quote}</p>
              </blockquote>
            )}
            <ol
              aria-label="Conversation"
              className={`mx-auto flex w-full max-w-3xl flex-col gap-8 px-4 pb-48 ${start ? "pt-8" : "pt-24"}`}
            >
              {turns.map((t) => (
                <motion.li
                  key={t.id}
                  initial={still ? false : { opacity: 0, filter: "blur(6px)" }}
                  animate={{ opacity: 1, filter: "blur(0px)" }}
                  transition={{ duration: 0.35, ease: EASE }}
                  className="flex flex-col gap-6"
                >
                  <motion.div
                    initial={arrive(still, { y: 14, scale: 0.94 })}
                    animate={settled}
                    transition={spring.soft}
                    style={{ originX: 1, originY: 1 }}
                    className="flex max-w-[85%] flex-col items-end gap-2 self-end"
                  >
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
                    {t.question && (
                      <p className="whitespace-pre-wrap rounded-3xl rounded-br-lg bg-shu px-4 py-2.5 text-[15px] leading-6 text-white shadow-[0_8px_24px_-12px_rgb(120_28_53/0.6)]">
                        {t.question}
                      </p>
                    )}
                    <MovedTo thread={threadOfQuestion(t)} />
                  </motion.div>
                  {t.answer && (
                    <Reply
                      answer={t.answer}
                      busy={busy}
                      revealFrom={t.reveal}
                      onConfirm={(approve) => answerHeld(t, approve)}
                      onChoose={t.id === turns[turns.length - 1]?.id ? (choice) => send(choice) : undefined}
                      threads={threadsOf(t)}
                    />
                  )}
                  {t.error && (
                    <AssistantRow mood="sad" enter>
                      <div className="flex flex-wrap items-center gap-3">
                        <p role="alert" className="text-sm text-bad">
                          {t.error}
                        </p>
                        {!busy && t.question && !HELD.has(t.question) && (
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
                  {!t.answer && !t.error && busy && t.id === liveId && <Running lines={live.lines} draft={live.draft} onShown={onShown} fly={turns.length === 1} />}
                </motion.li>
              ))}
            </ol>
          </div>
          <div className="pointer-events-none absolute inset-x-0 bottom-0">
            <div className="pointer-events-auto mx-auto w-full max-w-3xl px-4 pb-4">
              {composer}
              <p className="mt-2 text-center text-xs text-dim">Sparky can be wrong. Check the sources it links.</p>
            </div>
          </div>
        </main>
      )}
    </ChatFrame>
  );
};

export default Chat;
