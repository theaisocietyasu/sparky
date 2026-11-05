import { Bell as BellIcon } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { notices, readNotices } from "../chat/client";
import type { Notice } from "../chat/types";

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

/** The bell in the sidebar: a dot for unread reminders, and their list. Opening it marks them read. */
const Bell = () => {
  const [items, setItems] = useState<Notice[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  const still = useReducedMotion() ?? false;
  const box = useRef<HTMLDivElement>(null);

  const load = useCallback(() => {
    notices()
      .then((n) => {
        setItems(n.notifications);
        setUnread(n.unread);
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
              <p className="px-3 py-6 text-center text-sm text-dim">No reminders yet. Ask Sparky to remind you of something.</p>
            ) : (
              <ul className="max-h-80 overflow-y-auto">
                {items.map((n) => (
                  <li key={n.id} className="rounded-lg px-3 py-2.5">
                    <p className="text-sm">{n.text}</p>
                    <p className="mt-0.5 text-xs text-dim">{ago(n.created_at)}</p>
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
