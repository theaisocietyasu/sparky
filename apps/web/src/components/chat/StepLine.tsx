import {
  Ban,
  Brain,
  CircleCheck,
  CircleX,
  FileArchive,
  Hand,
  LoaderCircle,
  MessageSquareText,
  Paperclip,
  PenLine,
  RefreshCw,
  Trash2,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import { Fragment } from "react";
import type { StepKind } from "../../chat/step";

const ICONS: Record<StepKind, [LucideIcon, string]> = {
  thinking: [LoaderCircle, "animate-spin text-dim"],
  thought: [MessageSquareText, "text-dim"],
  tool: [Wrench, "text-accent"],
  done: [CircleCheck, "text-good"],
  failed: [CircleX, "text-bad"],
  memory: [Brain, "text-dim"],
  note: [PenLine, "text-dim"],
  forget: [Trash2, "text-dim"],
  blocked: [Ban, "text-bad"],
  approval: [Hand, "text-warn"],
  summary: [FileArchive, "text-dim"],
  file: [Paperclip, "text-dim"],
  retry: [RefreshCw, "text-warn"],
  step: [LoaderCircle, "text-dim"],
};

/** Words with each backticked name shown as code. */
const Words = ({ text }: { text: string }) => (
  <>
    {text.split(/(`[^`]+`)/g).map((part, i) =>
      part.startsWith("`") && part.endsWith("`") && part.length > 2 ? (
        <code key={i} className="rounded bg-sunk px-1 py-0.5 font-mono text-[0.8em] text-fg">
          {part.slice(1, -1)}
        </code>
      ) : (
        <Fragment key={i}>{part}</Fragment>
      ),
    )}
  </>
);

/** One progress step: an icon for its kind and its words, held to two lines with the rest on hover. */
const StepLine = ({ kind, text }: { kind: StepKind; text: string }) => {
  const [Icon, tone] = ICONS[kind];
  return (
    <span className="flex items-start gap-2.5">
      <Icon aria-hidden className={`mt-[3px] size-3.5 shrink-0 ${tone}`} />
      <span title={text} className={`line-clamp-2 [overflow-wrap:anywhere] ${kind === "thought" ? "italic" : ""}`}>
        <Words text={text} />
      </span>
    </span>
  );
};

export default StepLine;
