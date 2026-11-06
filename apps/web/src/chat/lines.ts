import type { Progress } from "./types";

/** One progress line on screen. */
export type Line = { key: string; text: string };

/** What a running turn shows: its steps, the draft being written, and a withdrawn draft kept on screen. */
export type LiveState = { lines: Line[]; draft: string; held: string; next: number };

/**
 * The state after one more progress event. A withdrawn draft stays on screen as held until the next
 * draft replaces it, so a reply never vanishes mid-turn.
 */
export const applyProgress = (state: LiveState, p: Progress): LiveState => {
  if (p.draft && (p.clear || !p.text)) return { ...state, draft: "", held: state.draft || state.held };
  if (p.draft) return { ...state, draft: p.text, held: "" };
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
