import type { TargetAndTransition, Transition } from "motion/react";

/** The springs every student page shares, so motion on one surface feels like motion on the next. */
export const spring = {
  /** Presses, toggles and small controls. */
  snap: { type: "spring", stiffness: 520, damping: 38, mass: 0.7 },
  /** Cards, messages and sheets. */
  soft: { type: "spring", stiffness: 280, damping: 30 },
  /** Layout changes: a list reflowing, the composer moving. */
  glide: { type: "spring", stiffness: 380, damping: 36 },
} satisfies Record<string, Transition>;

/** A strong ease-out for fades. */
export const EASE = [0.23, 1, 0.32, 1] as const;

/** How a message, card or row arrives: up, sharp and opaque out of a soft blur. */
export const arrive = (still: boolean, from: Partial<{ y: number; x: number; scale: number }> = {}): false | TargetAndTransition =>
  still ? false : { opacity: 0, y: from.y ?? 10, x: from.x ?? 0, scale: from.scale ?? 0.98, filter: "blur(6px)" };

/** Where every arrival ends. */
export const settled: TargetAndTransition = { opacity: 1, y: 0, x: 0, scale: 1, filter: "blur(0px)" };
