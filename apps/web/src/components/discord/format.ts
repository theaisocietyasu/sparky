/**
 * The text of a Sparky reply, built the way the bot builds it.
 * Mirrors apps/discord/src/render/card.rs and TraceEvent::progress in the engine.
 */

/** Header while a turn runs and no answer is being written. */
const THINKING = "**Sparky is working on it…**";

/** Header once the answer is being written. */
const ANSWERING = "**Sparky is answering…**";

/** Frames the header spinner cycles through, one per edit. */
const SPINNER = ["◐", "◓", "◑", "◒"];

/** Characters of arguments and result on a progress line, progress_detail_chars in sparky.toml. */
const DETAIL = 160;

/** Characters of a thought on a progress line, progress_thought_chars in sparky.toml. */
const THOUGHT = 600;

/** Arguments of one tool call, in the key order the engine prints them. */
export type Args = Record<string, string>;

/** One tool call of a turn: the tool, its arguments, and what came back. */
export type ToolCall = { tool: string; args: Args; output: string };

/** Text as one line, with runs of whitespace collapsed. */
const oneLine = (text: string) => text.split(/\s+/).filter(Boolean).join(" ");

/** Text held to limit characters, with an ellipsis when it was cut. */
const clip = (text: string, limit: number) => {
  const chars = Array.from(text);
  if (chars.length <= limit) return text;
  return `${chars.slice(0, limit - 1).join("").trimEnd()}…`;
};

/** The arguments a call was made with, sorted by key as serde_json prints them. */
const callArguments = (args: Args) => {
  const shown = Object.keys(args)
    .sort()
    .map((key) => `${key}: ${args[key]}`);
  return shown.length ? ` (${clip(oneLine(shown.join(", ")), DETAIL)})` : "";
};

/** What a tool is shown as while it runs. */
const running = (tool: string) =>
  tool === "search" ? "searching" : "running";

/** The line of a model step that has not answered yet. */
export const thinkingStep = () => "\u{1f914} thinking";

/** The line of a model step that showed its thought. */
export const thoughtStep = (text: string) => `\u{1f4ad} ${clip(oneLine(text), THOUGHT)}`;

/** The line of a tool call that is still running. */
export const toolStarted = (tool: string, args: Args) =>
  `\u{1f527} \`${tool}\`${callArguments(args)} — ${running(tool)}`;

/** The line of a tool call that returned. */
export const toolDone = (tool: string, args: Args, output: string) => {
  const head = `\`${tool}\`${callArguments(args)}`;
  const shown = clip(oneLine(output), DETAIL);
  return shown ? `✅ ${head} → ${shown}` : `✅ ${head} — nothing came back`;
};

/** What search returns when a source answered live: its key and the page it fetched. */
export const liveResult = (source: string, url: string, text: string) => `Live result from ${source} (${url}):\n\n${text}`;

/** What search returns for one stored page. */
export const storedResult = (title: string, url: string, age: string, text: string) =>
  `[1] ${title} - ${url} (stored copy, fetched ${age})\n${text}`;

/** One step as a small grey line. */
const bullet = (step: string) => `-# ${step}`;

/** The message while the turn runs: spinner header, steps, then the answer so far. */
export const runningCard = (steps: string[], frame: number, draft?: string) => {
  const title = draft ? ANSWERING : THINKING;
  const head = `${SPINNER[frame % SPINNER.length]} ${title}`;
  const out = [head, ...steps.map(bullet)].join("\n");
  return draft ? `${out}\n\n${draft}` : out;
};

/** The finished turn: steps, then the answer, then the unlinked sources. */
export const finishedCard = (steps: string[], answer: string, alsoFrom: string[] = []) =>
  [steps.map(bullet).join("\n"), answer.trim(), alsoFrom.length ? `**\u{1f4da} Also from** ${alsoFrom.join(", ")}` : ""]
    .filter(Boolean)
    .join("\n\n");

/** The label a source button carries: its title, held to 40 characters. */
export const sourceLabel = (title: string) => {
  const t = title.replace(/_/g, " ").trim();
  return Array.from(t).length <= 40 ? t : `${Array.from(t).slice(0, 39).join("").trimEnd()}…`;
};

/** The thread a mention opens is named after the question, at most this many characters. */
export const threadName = (question: string, max = 100) => {
  const line = oneLine(question);
  return line ? clip(line, max) : "Question";
};
