import { Brain, ShieldCheck } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";
import type { Answer } from "../../chat/types";
import { useSmoothText } from "../../chat/useSmoothText";
import { host, isWeb } from "../../chat/links";
import Markdown, { CopyButton } from "./Markdown";

/** The Sparky avatar beside each answer. */
export const Avatar = () => (
  <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-full border border-edge bg-raised">
    <img src="/brand/sparkyai-logo.png" alt="" width={717} height={779} className="h-5 w-auto" />
  </span>
);

/** An assistant turn: the avatar and its content. */
export const AssistantRow = ({ children }: { children: ReactNode }) => (
  <div className="flex gap-4">
    <Avatar />
    <div className="min-w-0 flex-1 pt-1">{children}</div>
  </div>
);

/**
 * One answer: its text, its sources, the memories it used, and approval buttons when it asks.
 * With revealFrom set, the text types out from that character and the rest fades in after it.
 */
const Reply = ({
  answer,
  onConfirm,
  busy,
  revealFrom,
}: {
  answer: Answer;
  onConfirm: (approve: boolean) => void;
  busy: boolean;
  revealFrom?: number;
}) => {
  const still = useReducedMotion() ?? false;
  const { text, done } = useSmoothText(answer.text, { from: revealFrom ?? 0, off: revealFrom === undefined || still });
  const sources = answer.citations ?? [];
  const used = answer.memories ?? [];
  return (
    <AssistantRow>
      <Markdown text={text} streaming={!done} />
      {done && <Extras answer={answer} onConfirm={onConfirm} busy={busy} sources={sources} used={used} animate={revealFrom !== undefined && !still} />}
    </AssistantRow>
  );
};

const Extras = ({
  answer,
  onConfirm,
  busy,
  sources,
  used,
  animate,
}: {
  answer: Answer;
  onConfirm: (approve: boolean) => void;
  busy: boolean;
  sources: NonNullable<Answer["citations"]>;
  used: string[];
  animate: boolean;
}) => {
  return (
    <motion.div initial={animate ? { opacity: 0, y: 4 } : false} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.25, ease: "easeOut" }}>
      {answer.confirmation && (
        <div className="mt-4 rounded-xl border border-edge bg-raised p-4">
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
            const chip = "inline-flex h-8 items-center gap-2 rounded-lg border border-edge bg-raised px-2.5 text-xs";
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
        <CopyButton text={answer.text} label="Copy answer" />
        {used.length > 0 && (
          <details className="group text-xs text-dim">
            <summary className="inline-flex h-7 cursor-pointer list-none items-center gap-1.5 rounded-md px-2 hover:bg-sunk hover:text-fg">
              <Brain className="size-3.5" />
              Memories used: {used.length}
            </summary>
            <ul className="mt-1 space-y-1 rounded-lg border border-edge bg-raised p-3 text-fg">
              {used.map((m) => (
                <li key={m}>{m}</li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </motion.div>
  );
};

export default Reply;
