import { MessagesSquare } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { NavLink } from "react-router-dom";
import { notices } from "../chat/client";
import Sparky from "../components/sparky/Sparky";
import { cue, useCue } from "../components/sparky/cue";
import ChatList from "./ChatList";
import { Account } from "./Frame";
import { spring } from "./motion";

const EVERY_MS = 60_000;

/** How many of Sparky's own messages the student has not seen, checked every minute. Sparky hops when one arrives. */
const useUnread = (seen: number) => {
  const [unread, setUnread] = useState(0);
  const last = useRef<number | null>(null);
  useEffect(() => {
    const load = () =>
      notices()
        .then((n) => {
          if (last.current !== null && n.unread > last.current) cue();
          last.current = n.unread;
          setUnread(n.unread);
        })
        .catch(() => undefined);
    load();
    const timer = window.setInterval(() => document.visibilityState === "visible" && load(), EVERY_MS);
    return () => window.clearInterval(timer);
  }, [seen]);
  return unread;
};

/** Two slow blurred washes that drift behind the glass. */
const Aura = () => (
  <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
    <div className="aura-blob -right-40 -top-56 size-[42rem] bg-[var(--app-aura-a)]" />
    <div className="aura-blob aura-blob-late -bottom-64 -left-48 size-[38rem] bg-[var(--app-aura-b)]" />
  </div>
);

/**
 * The frame of the student pages: a floating bar with Sparky, which links home and shows a dot for unread
 * messages, crumb (where a thread sits), the threads sheet and the profile menu. seen changes when home has been
 * read, so the dot clears.
 */
const ChatFrame = ({ chats, seen = 0, crumb, children }: { chats?: number; seen?: number; crumb?: ReactNode; children: ReactNode }) => {
  const unread = useUnread(seen);
  const mood = useCue();
  const still = useReducedMotion() ?? false;
  const [open, setOpen] = useState(false);

  return (
    <div className="relative isolate flex h-dvh flex-col bg-canvas text-fg">
      <Aura />
      <header className="pointer-events-none absolute inset-x-0 top-0 z-30 flex justify-center px-3 pt-3">
        <motion.nav
          aria-label="Chats"
          initial={still ? false : { y: -14, opacity: 0, filter: "blur(6px)" }}
          animate={{ y: 0, opacity: 1, filter: "blur(0px)" }}
          transition={spring.soft}
          className="bar pointer-events-auto flex h-12 w-full max-w-3xl items-center gap-1 rounded-full pl-1.5 pr-1.5"
        >
          <NavLink
            to="/chat"
            end
            aria-label={unread > 0 ? `Home, ${unread} new` : "Home"}
            className="flex h-9 items-center gap-2 rounded-full px-2.5 text-[15px] font-semibold tracking-tight transition-colors hover:bg-sunk/70"
          >
            <Sparky mood={mood} className="h-6 w-auto" />
            Sparky
            <AnimatePresence>
              {unread > 0 && (
                <motion.span
                  key="dot"
                  initial={{ scale: 0 }}
                  animate={{ scale: 1 }}
                  exit={{ scale: 0 }}
                  transition={spring.snap}
                  className="size-2 rounded-full bg-kin shadow-[0_0_0_3px_rgb(242_183_5/0.2)]"
                />
              )}
            </AnimatePresence>
          </NavLink>
          <AnimatePresence initial={false}>
            {crumb && (
              <motion.div
                key="crumb"
                initial={still ? false : { opacity: 0, x: -6, filter: "blur(4px)" }}
                animate={{ opacity: 1, x: 0, filter: "blur(0px)" }}
                exit={still ? undefined : { opacity: 0, x: -6, filter: "blur(4px)" }}
                transition={spring.soft}
              >
                {crumb}
              </motion.div>
            )}
          </AnimatePresence>
          <div className="flex-1" />
          <motion.button
            type="button"
            aria-label="Threads"
            aria-expanded={open}
            aria-controls="threads-sheet"
            onClick={() => setOpen((v) => !v)}
            whileTap={still ? undefined : { scale: 0.95 }}
            transition={spring.snap}
            className={`inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm transition-colors ${
              open ? "bg-sunk text-fg" : "text-dim hover:bg-sunk/70 hover:text-fg"
            }`}
          >
            <MessagesSquare className="size-4" />
            <span className="hidden sm:inline">Threads</span>
          </motion.button>
          <Account compact />
        </motion.nav>
      </header>
      <ChatList open={open} onClose={() => setOpen(false)} version={chats} />
      {children}
    </div>
  );
};

export default ChatFrame;
