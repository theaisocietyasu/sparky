import type { Answer } from "../../chat/types";

/** Whether a source link is a web address, so no other scheme becomes a link. */
const web = (url?: string | null): url is string => !!url && /^https?:\/\//i.test(url);

/** One answer: its text, its sources, the memories it used, and approval buttons when it asks. */
const Reply = ({
  answer,
  onConfirm,
  busy,
}: {
  answer: Answer;
  onConfirm: (approve: boolean) => void;
  busy: boolean;
}) => {
  const sources = answer.citations ?? [];
  const used = answer.memories ?? [];
  return (
    <div className="rounded-md border border-ink/10 bg-white/60 px-4 py-3">
      <p className="whitespace-pre-wrap text-[15px] leading-relaxed">{answer.text}</p>
      {sources.length > 0 && (
        <ol aria-label="Sources" className="mt-3 flex flex-wrap gap-2 text-xs">
          {sources.map((s, i) => (
            <li key={`${s.key}-${i}`}>
              {web(s.url) ? (
                <a href={s.url} target="_blank" rel="noreferrer" className="rounded-full border border-ink/15 px-2 py-0.5 hover:border-shu">
                  [{i + 1}] {s.title}
                </a>
              ) : (
                <span className="rounded-full border border-ink/15 px-2 py-0.5">
                  [{i + 1}] {s.title}
                </span>
              )}
            </li>
          ))}
        </ol>
      )}
      {used.length > 0 && (
        <details className="mt-2 text-xs text-ink-soft">
          <summary>Memories used: {used.length}</summary>
          <ul className="mt-1 list-disc pl-5">
            {used.map((m) => (
              <li key={m}>{m}</li>
            ))}
          </ul>
        </details>
      )}
      {answer.confirmation && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <p className="w-full text-sm font-medium">{answer.confirmation.summary}</p>
          <button
            type="button"
            disabled={busy}
            onClick={() => onConfirm(true)}
            className="h-9 rounded-full bg-shu px-4 text-sm font-semibold text-paper disabled:opacity-50"
          >
            Approve
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => onConfirm(false)}
            className="h-9 rounded-full border border-ink/20 px-4 text-sm disabled:opacity-50"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
};

export default Reply;
