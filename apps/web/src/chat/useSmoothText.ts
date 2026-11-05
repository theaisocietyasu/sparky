import { useEffect, useState } from "react";

/** Milliseconds between two visible steps; about 30 a second keeps the markdown parse cheap. */
const STEP_MS = 33;

/** How many characters one step reveals: a floor, plus a share of what is still hidden so a long backlog catches up. */
export const stepSize = (hidden: number) => Math.max(3, Math.ceil(hidden / 12));

/**
 * The part of target to show now. It grows toward target a few characters at a time, starting at from.
 * With off set, the whole target shows at once.
 */
export const useSmoothText = (target: string, { from = 0, off = false }: { from?: number; off?: boolean } = {}) => {
  const [shown, setShown] = useState(() => Math.min(from, target.length));
  const at = off ? target.length : Math.min(shown, target.length);

  useEffect(() => {
    if (off || at >= target.length) return;
    const timer = setTimeout(() => setShown(at + stepSize(target.length - at)), STEP_MS);
    return () => clearTimeout(timer);
  }, [at, target, off]);

  return { text: target.slice(0, at), done: at >= target.length };
};
