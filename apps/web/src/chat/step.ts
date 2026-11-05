/** What a progress line is about, read from the mark the engine puts in front of its text. */
export type StepKind =
  | "thinking"
  | "thought"
  | "tool"
  | "done"
  | "failed"
  | "memory"
  | "note"
  | "forget"
  | "blocked"
  | "approval"
  | "summary"
  | "file"
  | "retry"
  | "step";

/** One progress line split into its kind and its words. */
export type Step = { kind: StepKind; text: string };

const MARKS: Record<string, StepKind> = {
  "\u{1f914}": "thinking",
  "\u{1f4ad}": "thought",
  "\u{1f527}": "tool",
  "\u{2705}": "done",
  "\u{274c}": "failed",
  "\u{1f9e0}": "memory",
  "\u{1f4dd}": "note",
  "\u{1f5d1}": "forget",
  "\u{1f6d1}": "blocked",
  "\u{1f6ab}": "blocked",
  "\u{270b}": "approval",
  "\u{1f5dc}": "summary",
  "\u{1f4ce}": "file",
  "\u{1f504}": "retry",
};

const LEADING_MARK = /^(\p{Extended_Pictographic})\u{fe0f}?\s*/u;

/** The kind and words of a progress line; a line with no known mark is a plain step. */
export const parseStep = (line: string): Step => {
  const match = LEADING_MARK.exec(line);
  if (!match) return { kind: "step", text: line };
  return { kind: MARKS[match[1]] ?? "step", text: line.slice(match[0].length) };
};
