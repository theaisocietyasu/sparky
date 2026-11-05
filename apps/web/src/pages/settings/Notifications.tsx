import { useEffect, useState } from "react";
import { nudgePrefs, saveNudgePrefs } from "../../chat/client";
import type { NudgeKind, NudgePrefs } from "../../chat/types";
import Reminders from "./Reminders";
import { NUDGES } from "../../app/nudges";

const HOURS = Array.from({ length: 24 }, (_, h) => h);

/** An hour of the day as a clock reads it. */
const clock = (h: number) => new Date(2000, 0, 1, h).toLocaleTimeString(undefined, { hour: "numeric" });

/** An on and off switch for one check-in kind. */
const Toggle = ({ on, onChange, label }: { on: boolean; onChange: (on: boolean) => void; label: string }) => (
  <button
    type="button"
    role="switch"
    aria-checked={on}
    aria-label={label}
    onClick={() => onChange(!on)}
    className={`relative h-5 w-9 shrink-0 rounded-full transition-colors duration-200 ${on ? "bg-kin" : "bg-edge"}`}
  >
    <span
      className={`absolute left-0.5 top-0.5 size-4 rounded-full bg-white shadow-sm transition-transform duration-200 ${on ? "translate-x-4" : ""}`}
    />
  </button>
);

/** Settings, Notifications: which check-ins Sparky sends, quiet hours, and the student's reminders. */
const Notifications = () => {
  const [prefs, setPrefs] = useState<NudgePrefs | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    nudgePrefs()
      .then(setPrefs)
      .catch(() => setError("Could not load your check-in settings."));
  }, []);

  const save = (next: NudgePrefs) => {
    const before = prefs;
    setPrefs(next);
    saveNudgePrefs(next)
      .then(() => setError(null))
      .catch(() => {
        setPrefs(before);
        setError("Could not save that change.");
      });
  };

  const flip = (kind: NudgeKind, on: boolean) => {
    if (!prefs) return;
    const off = on ? prefs.off.filter((k) => k !== kind) : [...prefs.off, kind];
    save({ ...prefs, off });
  };

  return (
    <div className="space-y-10">
      <section aria-labelledby="settings-title">
        <h2 id="settings-title" className="text-lg font-semibold tracking-tight">
          Notifications
        </h2>
        <p className="mt-1 text-sm text-dim">Sparky checks in on its own, here and in your Discord DMs. At most a few a day.</p>
        {error && (
          <p role="alert" className="mt-4 text-sm text-bad">
            {error}
          </p>
        )}
        {!prefs && !error && <div aria-hidden className="mt-6 h-40 animate-pulse rounded-2xl bg-sunk" />}
        {prefs && (
          <>
            <ul aria-label="Check-ins" className="glass mt-6 divide-y divide-edge overflow-hidden rounded-2xl">
              {NUDGES.map((n) => (
                <li key={n.kind} className="flex items-center gap-4 px-4 py-3.5">
                  <div className="min-w-0 flex-1">
                    <p className="text-[15px]">{n.label}</p>
                    <p className="mt-0.5 text-xs text-dim">{n.detail}</p>
                  </div>
                  <Toggle on={!prefs.off.includes(n.kind)} onChange={(on) => flip(n.kind, on)} label={n.label} />
                </li>
              ))}
            </ul>
            <div className="glass mt-4 flex flex-wrap items-center gap-3 rounded-2xl px-4 py-3.5 text-[15px]">
              <span className="flex-1">Quiet hours</span>
              <label className="sr-only" htmlFor="quiet-start">
                Quiet from
              </label>
              <select
                id="quiet-start"
                value={prefs.quiet_start}
                onChange={(e) => save({ ...prefs, quiet_start: Number(e.target.value) })}
                className="rounded-lg border border-edge bg-transparent px-2 py-1 text-sm"
              >
                {HOURS.map((h) => (
                  <option key={h} value={h}>
                    {clock(h)}
                  </option>
                ))}
              </select>
              <span className="text-sm text-dim">to</span>
              <label className="sr-only" htmlFor="quiet-end">
                Quiet until
              </label>
              <select
                id="quiet-end"
                value={prefs.quiet_end}
                onChange={(e) => save({ ...prefs, quiet_end: Number(e.target.value) })}
                className="rounded-lg border border-edge bg-transparent px-2 py-1 text-sm"
              >
                {HOURS.map((h) => (
                  <option key={h} value={h}>
                    {clock(h)}
                  </option>
                ))}
              </select>
            </div>
          </>
        )}
      </section>
      <Reminders />
    </div>
  );
};

export default Notifications;
