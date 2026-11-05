import { CalendarClock, type LucideIcon, SlidersHorizontal, Trash2, UserRound } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import type { Memory } from "../chat/types";

type Kind = { label: string; Icon: LucideIcon; tone: string };

const KINDS: Record<string, Kind> = {
  fact: { label: "Facts", Icon: UserRound, tone: "border-accent/40 bg-accent/10 text-accent" },
  preference: { label: "Preferences", Icon: SlidersHorizontal, tone: "border-good/40 bg-good/10 text-good" },
  task: { label: "Tasks", Icon: CalendarClock, tone: "border-warn/40 bg-warn/10 text-warn" },
};

const kindOf = (kind: string): Kind => KINDS[kind] ?? { label: kind, Icon: UserRound, tone: "border-edge bg-sunk text-dim" };

/** Memories grouped by kind, known kinds first in a fixed order. */
const groups = (items: Memory[]): [string, Memory[]][] => {
  const order = Object.keys(KINDS);
  const byKind = new Map<string, Memory[]>();
  for (const m of items) byKind.set(m.kind, [...(byKind.get(m.kind) ?? []), m]);
  return [...byKind].sort(([a], [b]) => (order.indexOf(a) + 1 || 99) - (order.indexOf(b) + 1 || 99));
};

/** The memories as a tree: the student, one branch per kind, one leaf per memory. */
const MemoryMap = ({ items, onForget }: { items: Memory[]; onForget: (number: number) => void }) => {
  const still = useReducedMotion() ?? false;
  return (
    <div className="mt-8 overflow-x-auto pb-2">
      <div className="flex min-w-max items-center">
        <div className="grid size-10 shrink-0 sm:size-12 place-items-center rounded-full bg-accent text-sm font-semibold text-accent-fg">You</div>
        <ul className="tree" aria-label="Memory map">
          {groups(items).map(([kind, list]) => {
            const { label, Icon, tone } = kindOf(kind);
            return (
              <li key={kind}>
                <div title={label} className={`flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1.5 text-sm font-medium sm:min-w-36 sm:px-3 ${tone}`}>
                  <Icon className="size-4" />
                  <span className="hidden sm:inline">{label}</span>
                  <span className="tabular-nums opacity-70">{list.length}</span>
                </div>
                <ul className="tree" aria-label={label}>
                  <AnimatePresence initial={false}>
                    {list.map((m) => (
                      <motion.li
                        key={m.id}
                        layout={!still}
                        exit={still ? undefined : { opacity: 0 }}
                        transition={{ duration: 0.2 }}
                      >
                        <div className="group flex w-48 items-start gap-2 rounded-xl border border-edge bg-raised px-3 py-2 sm:w-80">
                          <div className="min-w-0 flex-1">
                            <p className="text-sm">{m.text}</p>
                            <p className="mt-1 flex flex-wrap gap-2 text-xs text-dim">
                              <span className="tabular-nums">#{m.number}</span>
                              <span>{m.created_at}</span>
                              {m.due && <span className="text-warn">due {m.due}</span>}
                            </p>
                          </div>
                          <button
                            type="button"
                            onClick={() => onForget(m.number)}
                            aria-label={`Forget memory ${m.number}`}
                            className="grid size-7 shrink-0 place-items-center rounded-lg text-dim opacity-60 transition hover:bg-sunk hover:text-bad group-hover:opacity-100 focus-visible:opacity-100"
                          >
                            <Trash2 className="size-3.5" />
                          </button>
                        </div>
                      </motion.li>
                    ))}
                  </AnimatePresence>
                </ul>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
};

export default MemoryMap;
