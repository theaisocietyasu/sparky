import { ShieldCheck } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";
import { splitChoices } from "../../chat/choices";
import type { Answer } from "../../chat/types";
import { useSmoothText } from "../../chat/useSmoothText";
import { host, isWeb } from "../../chat/links";
import Sparky, { type Mood } from "../sparky/Sparky";
import Markdown, { CopyButton } from "./Markdown";

/** Sparky beside each answer, at work while the turn runs. */
export const Avatar = ({ mood = "idle" }: { mood?: Mood }) => (
  <span aria-hidden className="glass grid size-8 shrink-0 place-items-center rounded-full">
    <Sparky mood={mood} still={mood === "idle"} className="h-5 w-auto" />
  </span>
);

/** An assistant turn: the avatar and its content. */
export const AssistantRow = ({ mood, children }: { mood?: Mood; children: ReactNode }) => (
  <div className="flex gap-4">
    <Avatar mood={mood} />
    <div className="min-w-0 flex-1 pt-1">{children}</div>
  </div>
);

/**
 * One answer: its text, its sources, approval buttons when it asks, and choices to tap when it
 * asked a question and onChoose is set. With revealFrom set, the text types out from that character
 * and the rest fades in after it.
 */
const Reply = ({
  answer,
  onConfirm,
  onChoose,
  busy,
  revealFrom,
}: {
  answer: Answer;
  onConfirm: (approve: boolean) => void;
  onChoose?: (choice: string) => void;
  busy: boolean;
  revealFrom?: number;
}) => {
  const still = useReducedMotion() ?? false;
  const { text, done } = useSmoothText(answer.text, { from: revealFrom ?? 0, off: revealFrom === undefined || still });
  const shown = splitChoices(text).text;
  const { choices } = splitChoices(answer.text);
  const sources = answer.citations ?? [];
  const animate = revealFrom !== undefined && !still;
  return (
    <AssistantRow>
      <Markdown text={shown} streaming={!done} />
      {done && onChoose && choices.length > 0 && (
        <ul aria-label="Choices" className="mt-3 flex flex-wrap gap-2">
          {choices.map((c, i) => (
            <motion.li
              key={c}
              initial={animate ? { opacity: 0, y: 6 } : false}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.25, ease: "easeOut", delay: i * 0.05 }}
            >
              <button
                type="button"
                disabled={busy}
                onClick={() => onChoose(c)}
                className="glass h-9 rounded-full px-3.5 text-sm font-medium transition hover:-translate-y-px hover:text-accent disabled:opacity-50"
              >
                {c}
              </button>
            </motion.li>
          ))}
        </ul>
      )}
      {done && <Extras answer={answer} onConfirm={onConfirm} busy={busy} sources={sources} animate={animate} />}
    </AssistantRow>
  );
};

const Extras = ({
  answer,
  onConfirm,
  busy,
  sources,
  animate,
}: {
  answer: Answer;
  onConfirm: (approve: boolean) => void;
  busy: boolean;
  sources: NonNullable<Answer["citations"]>;
  animate: boolean;
}) => {
  return (
    <motion.div initial={animate ? { opacity: 0, y: 4 } : false} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.25, ease: "easeOut" }}>
      {answer.confirmation && (
        <div className="glass mt-4 rounded-2xl p-4">
          <p className="flex items-center gap-2 text-sm font-medium">
            <ShieldCheck className="size-4 text-accent" />
            {answer.confirmation.summary}
          </p>
          <div className="mt-3 flex gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onConfirm(true)}
              className="h-9 rounded-lg bg-fg px-4 text-sm font-medium text-canvas hover:opacity-90 disabled:opacity-50"
            >
              Approve
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onConfirm(false)}
              className="h-9 rounded-lg border border-edge px-4 text-sm hover:bg-sunk disabled:opacity-50"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
      {sources.length > 0 && (
        <ol aria-label="Sources" className="mt-4 flex flex-wrap gap-2">
          {sources.map((s, i) => {
            const body = (
              <>
                <span className="grid size-4 place-items-center rounded bg-sunk text-[10px] font-semibold text-dim">{i + 1}</span>
                <span className="max-w-56 truncate">{s.title}</span>
                {isWeb(s.url) && <span className="text-dim">{host(s.url)}</span>}
              </>
            );
            const chip = "glass inline-flex h-8 items-center gap-2 rounded-lg px-2.5 text-xs";
            return (
              <li key={`${s.key}-${i}`}>
                {isWeb(s.url) ? (
                  <a href={s.url} target="_blank" rel="noreferrer" className={`${chip} hover:border-dim`}>
                    {body}
                  </a>
                ) : (
                  <span className={chip}>{body}</span>
                )}
              </li>
            );
          })}
        </ol>
      )}
      <div className="mt-2 flex flex-wrap items-center gap-1">
        <CopyButton text={splitChoices(answer.text).text} label="Copy answer" />
      </div>
    </motion.div>
  );
};

export default Reply;
