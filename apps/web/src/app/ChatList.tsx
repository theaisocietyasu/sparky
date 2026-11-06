import { Trash2 } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { NavLink, useNavigate, useParams } from "react-router-dom";
import { chats, deleteChat } from "../chat/client";
import type { ChatSummary } from "../chat/types";
import { arrive, EASE, settled, spring } from "./motion";

/** The student's threads in a sheet under the bar: open one, or delete it. Reloads when version changes. */
const ChatList = ({ open, onClose, version = 0 }: { open: boolean; onClose: () => void; version?: number }) => {
  const [items, setItems] = useState<ChatSummary[] | null>(null);
  const { id } = useParams();
  const navigate = useNavigate();
  const still = useReducedMotion() ?? false;
  const sheet = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    chats()
      .then((c) => live && setItems(c))
      .catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [version]);

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      if (!sheet.current?.contains(target) && !target.closest("[aria-controls='threads-sheet']")) onClose();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open, onClose]);

  const drop = async (chat: ChatSummary) => {
    try {
      await deleteChat(chat.id);
    } catch {
      return;
    }
    setItems((all) => all?.filter((c) => c.id !== chat.id) ?? null);
    if (chat.id === id) navigate("/chat");
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={sheet}
          id="threads-sheet"
          initial={still ? false : { opacity: 0, y: -8, scale: 0.97, filter: "blur(8px)" }}
          animate={{ opacity: 1, y: 0, scale: 1, filter: "blur(0px)" }}
          exit={still ? undefined : { opacity: 0, y: -6, scale: 0.98, filter: "blur(6px)", transition: { duration: 0.15, ease: EASE } }}
          transition={spring.soft}
          className="glass fixed inset-x-3 top-[4.25rem] z-40 mx-auto max-h-[min(28rem,70dvh)] max-w-3xl origin-top overflow-y-auto rounded-3xl p-2 sm:left-auto sm:right-[max(0.75rem,calc((100vw-48rem)/2))] sm:w-80"
        >
          {items && items.length > 0 ? (
            <ul aria-label="Threads" className="space-y-0.5">
              <AnimatePresence initial={!still}>
                {items.map((c, i) => (
                  <motion.li
                    key={c.id}
                    layout={!still}
                    initial={arrive(still, { y: 6 })}
                    animate={settled}
                    exit={still ? undefined : { opacity: 0, height: 0, transition: { duration: 0.18, ease: EASE } }}
                    transition={{ ...spring.soft, delay: Math.min(i, 8) * 0.025 }}
                    className="group relative"
                  >
                    <NavLink
                      to={`/chat/${c.id}`}
                      onClick={onClose}
                      className={({ isActive }) =>
                        `flex h-10 items-center rounded-2xl pl-3 pr-10 text-sm transition-colors ${
                          isActive ? "bg-sunk font-medium text-fg" : "text-dim hover:bg-sunk/70 hover:text-fg"
                        }`
                      }
                    >
                      <span className="truncate">{c.title}</span>
                    </NavLink>
                    <button
                      type="button"
                      onClick={() => drop(c)}
                      aria-label={`Delete thread ${c.title}`}
                      className="absolute right-1.5 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-full text-dim opacity-60 transition md:opacity-0 hover:text-bad focus-visible:opacity-100 group-hover:opacity-100"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </motion.li>
                ))}
              </AnimatePresence>
            </ul>
          ) : (
            <p className="px-3 py-6 text-center text-sm text-dim">{items ? "No threads yet. A new topic in home starts one." : "Loading"}</p>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
};

export default ChatList;
