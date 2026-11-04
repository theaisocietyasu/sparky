import { useCallback, useEffect, useState } from "react";
import { forget, memories as fetchMemories } from "../chat/client";
import type { Memory } from "../chat/types";
import Gated from "../components/chat/Gated";

/** The list of what Sparky remembers, with forget per item and for all. */
const List = () => {
  const [items, setItems] = useState<Memory[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmAll, setConfirmAll] = useState(false);

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
    <>
      <h1 className="font-serif text-2xl font-bold">Memories</h1>
      <p className="mt-1 text-sm text-ink-soft">What Sparky remembers about you, the same as /memories in Discord.</p>
      {error && (
        <p role="alert" className="mt-4 text-sm text-shu">
          {error}
        </p>
      )}
      {items && items.length === 0 && <p className="mt-6 text-sm text-ink-soft">Sparky remembers nothing about you.</p>}
      {items && items.length > 0 && (
        <>
          <ol aria-label="Memories" className="mt-6 divide-y divide-ink/10 rounded-md border border-ink/10">
            {items.map((m) => (
              <li key={m.id} className="flex items-start gap-3 px-4 py-3">
                <span className="w-6 shrink-0 text-sm text-ink-soft">{m.number}.</span>
                <div className="flex-1">
                  <p className="text-sm">{m.text}</p>
                  <p className="mt-0.5 text-xs text-ink-soft">
                    {m.kind}, {m.created_at}
                    {m.due ? `, due ${m.due}` : ""}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => drop(m.number)}
                  aria-label={`Forget memory ${m.number}`}
                  className="text-sm text-shu hover:text-shu-deep"
                >
                  Forget
                </button>
              </li>
            ))}
          </ol>
          <div className="mt-4 flex gap-2">
            {confirmAll ? (
              <>
                <button type="button" onClick={() => drop(null)} className="h-9 rounded-full bg-shu px-4 text-sm font-semibold text-paper">
                  Forget all {items.length}
                </button>
                <button type="button" onClick={() => setConfirmAll(false)} className="h-9 rounded-full border border-ink/20 px-4 text-sm">
                  Keep them
                </button>
              </>
            ) : (
              <button type="button" onClick={() => setConfirmAll(true)} className="h-9 rounded-full border border-ink/20 px-4 text-sm">
                Forget all
              </button>
            )}
          </div>
        </>
      )}
    </>
  );
};

/** /memories: what Sparky remembers about the signed-in student. */
const Memories = () => <Gated>{() => <List />}</Gated>;

export default Memories;
