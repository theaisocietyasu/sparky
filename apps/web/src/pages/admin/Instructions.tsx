import { useState } from "react";
import * as api from "../../admin/client";
import { button } from "../../admin/format";
import { refusal, useLoad } from "../../admin/useLoad";
import { Alert, Badge, Card, Loading, PageHeader } from "./ui";

/** /admin/instructions: the system instructions editor with save and reset. */
const Instructions = () => {
  const { data, error, reload, setError } = useLoad(api.instructions);
  const [draft, setDraft] = useState<string | null>(null);
  const text = draft ?? data?.text ?? "";
  const save = (value: string | null) =>
    api
      .setInstructions(value)
      .then(() => {
        setDraft(null);
        reload();
      })
      .catch((e: unknown) => setError(refusal(e)));
  const over = data ? text.length > data.max_chars : false;
  return (
    <section aria-label="Instructions">
      <PageHeader
        title="Instructions"
        description="The system instructions every turn starts with."
        actions={
          data && (
            <>
              <button type="button" className={button.secondary} disabled={!data.changed} onClick={() => save(null)}>
                Use the configured instructions
              </button>
              <button type="button" className={button.primary} disabled={draft === null || over} onClick={() => save(text)}>
                Save
              </button>
            </>
          )
        }
      />
      <Alert text={error} />
      <Card>
        {!data && !error && <Loading rows={6} />}
        {data && (
          <>
            <div className="flex items-center justify-between border-b border-edge px-4 py-2.5 text-sm">
              {data.changed ? <Badge tone="warn">Changed by a moderator</Badge> : <Badge>Configured in sparky.toml</Badge>}
              {draft !== null && <span className="text-xs text-dim">Unsaved changes</span>}
            </div>
            <label htmlFor="instructions" className="sr-only">
              Instructions
            </label>
            <textarea
              id="instructions"
              value={text}
              onChange={(e) => setDraft(e.target.value)}
              rows={18}
              className="block w-full resize-y bg-transparent p-4 font-mono text-[13px] leading-relaxed outline-none"
            />
            <p className={`border-t border-edge px-4 py-2 text-right text-xs tabular-nums ${over ? "text-bad" : "text-dim"}`}>
              {text.length} / {data.max_chars} characters
            </p>
          </>
        )}
      </Card>
    </section>
  );
};

export default Instructions;
