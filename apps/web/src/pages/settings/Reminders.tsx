import { AlarmClock, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { cancelReminder, reminders } from "../../chat/client";
import type { Reminder } from "../../chat/types";

/** A reminder time in the reader's own words. */
const when = (iso: string) =>
  new Date(iso).toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });

/** Settings, Reminders: what Sparky will remind the student of, with cancel per item. */
const Reminders = () => {
  const [items, setItems] = useState<Reminder[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    reminders()
      .then((r) => {
        setItems(r);
        setError(null);
      })
      .catch(() => setError("Could not load your reminders."));
  }, []);

  useEffect(load, [load]);

  const drop = async (id: string) => {
    try {
      await cancelReminder(id);
    } catch {
      setError("Could not cancel that reminder.");
    }
    load();
  };

  return (
    <section aria-labelledby="reminders-title">
      <h3 id="reminders-title" className="text-base font-semibold tracking-tight">
        Reminders
      </h3>
      <p className="mt-1 text-sm text-dim">Ask Sparky in a chat, for example: remind me to submit the lab at 9pm. It messages you on Discord and here.</p>
      {error && (
        <p role="alert" className="mt-4 text-sm text-bad">
          {error}
        </p>
      )}
      {!items && !error && <div aria-hidden className="mt-6 h-16 animate-pulse rounded-2xl bg-sunk" />}
      {items && items.length === 0 && (
        <div className="mt-6 flex flex-col items-center rounded-2xl border border-dashed border-edge px-6 py-12 text-center">
          <AlarmClock className="size-6 text-dim" />
          <p className="mt-3 text-sm font-medium">No reminders set</p>
        </div>
      )}
      {items && items.length > 0 && (
        <ul aria-label="Reminders" className="glass mt-6 divide-y divide-edge overflow-hidden rounded-2xl">
          {items.map((r) => (
            <li key={r.id} className="flex items-center gap-4 px-4 py-3.5">
              <AlarmClock className="size-4 shrink-0 text-dim" />
              <div className="min-w-0 flex-1">
                <p className="text-[15px]">{r.text}</p>
                <p className="mt-0.5 text-xs text-dim">
                  {when(r.due_at)}
                  {r.repeat !== "once" && `, repeats ${r.repeat}`}
                </p>
              </div>
              <button
                type="button"
                onClick={() => drop(r.id)}
                aria-label={`Cancel reminder ${r.text}`}
                className="grid size-8 shrink-0 place-items-center rounded-lg text-dim hover:bg-sunk hover:text-bad"
              >
                <X className="size-4" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

export default Reminders;
