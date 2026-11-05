import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect } from "react";
import type { Line } from "../../chat/lines";
import { useSmoothText } from "../../chat/useSmoothText";
import Markdown from "./Markdown";
import { AssistantRow } from "./Reply";

/** What the running turn shows: each progress line the engine sends, then the draft as it streams in. */
const Running = ({ lines, draft, onShown }: { lines: Line[]; draft: string; onShown: (text: string) => void }) => {
  const still = useReducedMotion() ?? false;
  const { text } = useSmoothText(draft, { off: still });
  useEffect(() => onShown(text), [text, onShown]);
  return (
    <AssistantRow>
      <div aria-live="polite" aria-label="Progress">
        {!draft && (
          <ul className="space-y-1.5 text-sm text-dim">
            <AnimatePresence initial={false}>
              {lines.map((l) => (
                <motion.li
                  key={l.key}
                  layout={!still}
                  initial={still ? false : { opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.2 }}
                  className="flex items-center gap-2"
                >
                  <span className="size-1.5 shrink-0 rounded-full bg-dim/50" />
                  {l.text}
                </motion.li>
              ))}
            </AnimatePresence>
            <li className="shimmer w-fit">{lines.length === 0 ? "Thinking" : "Working"}</li>
          </ul>
        )}
        {draft && <Markdown text={text} streaming />}
      </div>
    </AssistantRow>
  );
};

export default Running;
