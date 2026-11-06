import { Check, Eraser } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useState } from "react";
import { spring } from "../../app/motion";

const HOLD_MS = 3000;

/** Clears the home chat: the first press asks, a second press within a few seconds clears it. */
const ClearHome = ({ onClear, disabled }: { onClear: () => void; disabled?: boolean }) => {
  const [asking, setAsking] = useState(false);
  const still = useReducedMotion() ?? false;

  useEffect(() => {
    if (!asking) return;
    const timer = window.setTimeout(() => setAsking(false), HOLD_MS);
    return () => window.clearTimeout(timer);
  }, [asking]);

  const press = () => {
    if (!asking) return setAsking(true);
    setAsking(false);
    onClear();
  };

  return (
    <motion.button
      type="button"
      layout={!still}
      disabled={disabled}
      onClick={press}
      aria-label={asking ? "Confirm clear home chat" : "Clear home chat"}
      whileTap={still ? undefined : { scale: 0.95 }}
      transition={spring.snap}
      className={`inline-flex h-9 items-center gap-2 overflow-hidden rounded-full px-3 text-sm transition-colors disabled:opacity-40 ${
        asking ? "bg-bad/10 text-bad" : "text-dim hover:bg-sunk/70 hover:text-fg"
      }`}
    >
      <AnimatePresence initial={false} mode="popLayout">
        <motion.span
          key={asking ? "ask" : "idle"}
          initial={still ? false : { opacity: 0, y: 8, filter: "blur(4px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={still ? undefined : { opacity: 0, y: -8, filter: "blur(4px)" }}
          transition={spring.snap}
          className="inline-flex items-center gap-2"
        >
          {asking ? <Check className="size-4" /> : <Eraser className="size-4" />}
          <span className={asking ? "" : "hidden sm:inline"}>{asking ? "Clear?" : "Clear"}</span>
        </motion.span>
      </AnimatePresence>
    </motion.button>
  );
};

export default ClearHome;
