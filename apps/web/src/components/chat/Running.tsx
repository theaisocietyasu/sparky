import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect } from "react";
import type { Line } from "../../chat/lines";
import { splitChoices } from "../../chat/choices";
import { parseStep } from "../../chat/step";
import { useSmoothText } from "../../chat/useSmoothText";
import Markdown from "./Markdown";
import { AssistantRow } from "./Reply";

/** The words of the newest progress line, with its mark and any code ticks taken off. */
const latest = (lines: Line[]) => {
  const last = lines[lines.length - 1];
  if (!last) return "Thinking";
  return parseStep(last.text).text.replace(/`/g, "") || "Thinking";
};

/**
 * What the running turn shows: Sparky at work and one status line that changes in place as the engine
 * reports each step, then the draft as it streams in.
 */
const Running = ({ lines, draft, onShown, fly = false }: { lines: Line[]; draft: string; onShown: (text: string) => void; fly?: boolean }) => {
  const still = useReducedMotion() ?? false;
  const { text } = useSmoothText(draft, { off: still });
  useEffect(() => onShown(text), [text, onShown]);
  const status = latest(lines);
  return (
    <AssistantRow mood={draft ? "idle" : "thinking"} fly={fly} enter={!fly}>
      <div aria-live="polite" aria-label="Progress">
        {!draft && (
          <div className="relative h-6 overflow-hidden text-sm">
            <AnimatePresence initial={false} mode="popLayout">
              <motion.p
                key={status}
                initial={still ? false : { opacity: 0, y: 10, filter: "blur(4px)" }}
                animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                exit={still ? undefined : { opacity: 0, y: -10, filter: "blur(4px)" }}
                transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}
                title={status}
                className="shimmer truncate leading-6"
              >
                {status}
              </motion.p>
            </AnimatePresence>
          </div>
        )}
        {draft && <Markdown text={splitChoices(text).text} streaming />}
      </div>
    </AssistantRow>
  );
};

export default Running;
