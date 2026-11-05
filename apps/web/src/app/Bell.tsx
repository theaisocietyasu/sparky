import { Bell as BellIcon, ThumbsDown, ThumbsUp } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { notices, rateNotice, readNotices } from "../chat/client";
import type { Notice } from "../chat/types";
import Sparky from "../components/sparky/Sparky";
import { cue } from "../components/sparky/cue";
import { nudgeLabel } from "./nudges";

const EVERY_MS = 60_000;

/** How long ago a notification was made, in a few words. */
const ago = (iso: string, now = Date.now()) => {
  const minutes = Math.max(0, Math.round((now - new Date(iso).getTime()) / 60_000));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });
};

/** Thumbs on a check-in. Not useful switches that kind off and says so. */
const Rate = ({ notice, onRated }: { notice: Notice; onRated: (n: Notice, note: string | null) => void }) => {
  if (notice.kind === "reminder") return null;
  const rate = (helpful: boolean) => {
    if (helpful) cue();
    onRated({ ...notice, helpful }, null);
    rateNotice(notice.id, helpful)
      .then((r) => onRated({ ...notice, helpful }, r.turned_off ? `No more ${nudgeLabel(r.turned_off).toLowerCase()}. Turn it back on in Settings.` : null))
      .catch(() => onRated(notice, null));
  };
  const button = (helpful: boolean, label: string, Icon: typeof ThumbsUp) => (
    <button
      type="button"
      aria-label={label}
      aria-pressed={notice.helpful === helpful}
      onClick={() => rate(helpful)}
      className={`grid size-6 place-items-center rounded-md transition-colors hover:bg-sunk ${
        notice.helpful === helpful ? "text-kin" : "text-dim hover:text-fg"
      }`}
    >
      <Icon className="size-3.5" />
    </button>
  );
  return (
    <span className="ml-auto flex gap-0.5">
      {button(true, "Useful", ThumbsUp)}
      {button(false, "Not useful", ThumbsDown)}
    </span>
  );
};

/** The bell in the sidebar: a dot for unread notifications, and their list. Opening it marks them read. */
const Bell = () => {
  const [items, setItems] = useState<Notice[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  const [notes, setNotes] = useState<Record<string, string>>({});
  const still = useReducedMotion() ?? false;
  const box = useRef<HTMLDivElement>(null);
  const seen = useRef<number | null>(null);

  const load = useCallback(() => {
    notices()
      .then((n) => {
        setItems(n.notifications);
        setUnread(n.unread);
        if (seen.current !== null && n.unread > seen.current) cue();
        seen.current = n.unread;
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    load();
    const timer = window.setInterval(() => document.visibilityState === "visible" && load(), EVERY_MS);
    return () => window.clearInterval(timer);
  }, [load]);

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && unread > 0) {
      setUnread(0);
      seen.current = 0;
      readNotices().catch(() => undefined);
    }
  };

  return (
    <div ref={box} className="relative">
      <button
        type="button"
        onClick={toggle}
        aria-label={unread > 0 ? `Notifications, ${unread} new` : "Notifications"}
        aria-expanded={open}
        className="relative grid size-8 place-items-center rounded-lg text-dim transition-colors hover:bg-sunk/70 hover:text-fg"
      >
        <BellIcon className="size-4" />
        {unread > 0 && <span aria-hidden className="absolute right-1.5 top-1.5 size-2 rounded-full bg-kin ring-2 ring-side" />}
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            role="dialog"
            aria-label="Notifications"
            initial={still ? false : { opacity: 0, y: -4, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={still ? undefined : { opacity: 0, y: -4, scale: 0.98 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="glass absolute left-0 top-full z-50 mt-2 w-72 origin-top-left rounded-xl p-1.5"
          >
            {items.length === 0 ? (
              <div className="flex flex-col items-center px-3 py-6 text-center">
                <Sparky mood="sleepy" still className="h-10 w-auto" />
                <p className="mt-3 text-sm text-dim">Nothing yet. Sparky checks in here when something comes up.</p>
              </div>
            ) : (
              <ul className="max-h-80 overflow-y-auto">
                {items.map((n) => (
                  <li key={n.id} className="rounded-lg px-3 py-2.5">
                    <p className="whitespace-pre-line text-sm">{n.text}</p>
                    <div className="mt-1 flex items-center text-xs text-dim">
                      <span>{ago(n.created_at)}</span>
                      <Rate
                        notice={n}
                        onRated={(rated, note) => {
                          setItems((all) => all.map((x) => (x.id === rated.id ? rated : x)));
                          if (note) setNotes((all) => ({ ...all, [rated.id]: note }));
                        }}
                      />
                    </div>
                    {notes[n.id] && <p className="mt-1 text-xs text-dim">{notes[n.id]}</p>}
                  </li>
                ))}
              </ul>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

export default Bell;
