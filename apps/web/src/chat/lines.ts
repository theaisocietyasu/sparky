import type { Progress } from "./types";

/** One progress line on screen. */
export type Line = { key: string; text: string };

/** The progress lines and the draft answer after one more progress event. */
export const applyProgress = (
  state: { lines: Line[]; draft: string; next: number },
  p: Progress,
): { lines: Line[]; draft: string; next: number } => {
  if (p.draft) return { ...state, draft: p.text };
  if (p.slot !== undefined && p.clear) {
    return { ...state, lines: state.lines.filter((l) => l.key !== `slot:${p.slot}`) };
  }
  if (p.slot !== undefined) {
    const key = `slot:${p.slot}`;
    const at = state.lines.findIndex((l) => l.key === key);
    if (at !== -1) {
      const lines = state.lines.slice();
      lines[at] = { key, text: p.text };
      return { ...state, lines };
    }
    return { ...state, lines: [...state.lines, { key, text: p.text }] };
  }
  return { ...state, lines: [...state.lines, { key: `line:${state.next}`, text: p.text }], next: state.next + 1 };
};
