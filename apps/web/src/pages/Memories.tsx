import { Brain, List, Network, Trash2 } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import ChatFrame from "../app/ChatFrame";
import { forget, memories as fetchMemories } from "../chat/client";
import type { Memory } from "../chat/types";
import MemoryMap from "./MemoryMap";

type View = "map" | "list";
const VIEW_KEY = "sparky.memories.view";

const savedView = (): View => {
  try {
    return localStorage.getItem(VIEW_KEY) === "list" ? "list" : "map";
  } catch {
    return "map";
  }
};

/** /memories: what Sparky remembers about the signed-in student, with forget per item and for all. */
const Memories = () => {
  const still = useReducedMotion() ?? false;
  const [items, setItems] = useState<Memory[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmAll, setConfirmAll] = useState(false);
  const [view, setView] = useState<View>(savedView);

  const show = (next: View) => {
    setView(next);
    try {
      localStorage.setItem(VIEW_KEY, next);
    } catch {
      // The choice lasts for this visit only.
    }
  };

  const load = useCallback(() => {
    fetchMemories()
      .then((m) => {
        setItems(m);
        setError(null);
      })
      .catch(() => setError("Could not load your memories."));
  }, []);

  useEffect(load, [load]);

  const drop = async (number: number | null) => {
    try {
      await forget(number);
    } catch {
      setError("Could not forget that.");
    }
    setConfirmAll(false);
    load();
  };

  return (
    <ChatFrame>
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-3xl px-4 py-10">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">Memories</h1>
              <p className="mt-1 text-sm text-dim">What Sparky remembers about you. The same list as /memories in Discord.</p>
            </div>
            {items && items.length > 0 && (
              <div className="flex gap-2">
                <div role="group" aria-label="View" className="flex h-9 rounded-lg border border-edge p-0.5">
                  {(
                    [
                      ["map", "Map", Network],
                      ["list", "List", List],
                    ] as const
                  ).map(([key, label, Icon]) => (
                    <button
                      key={key}
                      type="button"
                      onClick={() => show(key)}
                      aria-pressed={view === key}
                      className={`flex items-center gap-1.5 rounded-md px-2.5 text-sm ${view === key ? "bg-sunk font-medium" : "text-dim hover:text-fg"}`}
                    >
                      <Icon className="size-4" />
                      {label}
                    </button>
                  ))}
                </div>
                {confirmAll ? (
                  <>
                    <button type="button" onClick={() => setConfirmAll(false)} className="h-9 rounded-lg border border-edge px-3 text-sm hover:bg-sunk">
                      Keep them
                    </button>
                    <button type="button" onClick={() => drop(null)} className="h-9 rounded-lg bg-bad px-3 text-sm font-medium text-white hover:opacity-90">
                      Forget all {items.length}
                    </button>
                  </>
                ) : (
                  <button type="button" onClick={() => setConfirmAll(true)} className="h-9 rounded-lg border border-edge px-3 text-sm hover:bg-sunk">
                    Forget all
                  </button>
                )}
              </div>
            )}
          </div>
          {error && (
            <p role="alert" className="mt-6 rounded-lg border border-bad/30 bg-bad/10 px-3 py-2 text-sm text-bad">
              {error}
            </p>
          )}
          {!items && !error && (
            <div className="mt-8 space-y-3" aria-hidden>
              {[0, 1, 2].map((i) => (
                <div key={i} className="h-16 animate-pulse rounded-xl bg-sunk" />
              ))}
            </div>
          )}
          {items && items.length === 0 && (
            <div className="mt-10 flex flex-col items-center rounded-2xl border border-dashed border-edge px-6 py-14 text-center">
              <Brain className="size-6 text-dim" />
              <p className="mt-3 text-sm font-medium">Sparky remembers nothing about you</p>
              <p className="mt-1 text-sm text-dim">Tell it in a chat, for example: remember that I study at Tempe.</p>
            </div>
          )}
          {items && items.length > 0 && view === "map" && <MemoryMap items={items} onForget={(n) => drop(n)} />}
          {items && items.length > 0 && view === "list" && (
            <ol aria-label="Memories" className="mt-8 divide-y divide-edge overflow-hidden rounded-2xl border border-edge bg-raised">
              <AnimatePresence initial={false}>
                {items.map((m) => (
                  <motion.li
                    key={m.id}
                    layout={!still}
                    exit={still ? undefined : { opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="group flex items-start gap-4 px-4 py-3.5"
                  >
                    <span className="mt-0.5 w-5 shrink-0 text-sm tabular-nums text-dim">{m.number}</span>
                    <div className="min-w-0 flex-1">
                      <p className="text-[15px]">{m.text}</p>
                      <p className="mt-1 flex flex-wrap items-center gap-2 text-xs text-dim">
                        <span className="rounded-md bg-sunk px-1.5 py-0.5 font-medium">{m.kind}</span>
                        <span>{m.created_at}</span>
                        {m.due && <span className="text-warn">due {m.due}</span>}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => drop(m.number)}
                      aria-label={`Forget memory ${m.number}`}
                      className="grid size-8 shrink-0 place-items-center rounded-lg text-dim opacity-60 transition hover:bg-sunk hover:text-bad group-hover:opacity-100 focus-visible:opacity-100"
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </motion.li>
                ))}
              </AnimatePresence>
            </ol>
          )}
        </div>
      </main>
    </ChatFrame>
  );
};

export default Memories;
