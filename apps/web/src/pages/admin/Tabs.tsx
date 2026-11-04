import { useState } from "react";
import * as api from "../../admin/client";
import { refusal, useLoad } from "../../admin/useLoad";

const Alert = ({ text }: { text: string | null }) =>
  text ? (
    <p role="alert" className="mt-3 text-sm text-shu">
      {text}
    </p>
  ) : null;

const button = "h-9 rounded-full px-4 text-sm disabled:opacity-50";
const primary = `${button} bg-shu font-semibold text-paper`;
const secondary = `${button} border border-ink/20`;

/** Every tool with an on and off switch. */
export const Tools = () => {
  const { data, error, reload, setError } = useLoad(api.tools);
  const flip = (name: string, on: boolean) =>
    api
      .switchTool(name, on)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  return (
    <section aria-label="Tools">
      <Alert text={error} />
      <ul className="divide-y divide-ink/10 rounded-md border border-ink/10">
        {data?.map((t) => (
          <li key={t.name} className="flex items-start gap-3 px-4 py-3">
            <div className="flex-1">
              <p className="font-mono text-sm">
                {t.name}
                {t.private && <span className="ml-2 font-sans text-xs text-ink-soft">direct messages only</span>}
              </p>
              <p className="mt-0.5 line-clamp-2 text-xs text-ink-soft">{t.description}</p>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={t.on} onChange={(e) => flip(t.name, e.target.checked)} aria-label={`Offer ${t.name}`} />
              {t.on ? "On" : "Off"}
            </label>
          </li>
        ))}
      </ul>
    </section>
  );
};

/** The instructions editor with save and reset. */
export const Instructions = () => {
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
  return (
    <section aria-label="Instructions">
      <Alert text={error} />
      {data && (
        <>
          <p className="text-sm text-ink-soft">
            {data.changed ? "A moderator changed these instructions." : "These are the configured instructions."}
          </p>
          <label htmlFor="instructions" className="sr-only">
            Instructions
          </label>
          <textarea
            id="instructions"
            value={text}
            onChange={(e) => setDraft(e.target.value)}
            rows={14}
            className="mt-2 w-full rounded-md border border-ink/20 bg-white p-3 font-mono text-sm outline-none focus:border-shu"
          />
          <p className="text-xs text-ink-soft">
            {text.length} / {data.max_chars} characters
          </p>
          <div className="mt-3 flex gap-2">
            <button type="button" className={primary} disabled={draft === null || text.length > data.max_chars} onClick={() => save(text)}>
              Save
            </button>
            <button type="button" className={secondary} disabled={!data.changed} onClick={() => save(null)}>
              Use the configured instructions
            </button>
          </div>
        </>
      )}
    </section>
  );
};

const describe = (c: api.Change) => {
  if (c.kind === "tool") return `${c.key} ${c.value === true ? "on" : "off"}`;
  if (c.value === null) return "Instructions reset to the configured ones";
  return `Instructions: ${String(c.value).slice(0, 80)}`;
};

/** Every change, newest first, with rollback. */
export const History = () => {
  const { data, error, reload, setError } = useLoad(api.history);
  const back = (id: number) =>
    api
      .rollback(id)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  return (
    <section aria-label="History">
      <Alert text={error} />
      {data?.length === 0 && <p className="text-sm text-ink-soft">No changes yet.</p>}
      <ol className="divide-y divide-ink/10 rounded-md border border-ink/10">
        {data?.map((c) => (
          <li key={c.id} className="flex items-start gap-3 px-4 py-3">
            <div className="flex-1">
              <p className="text-sm">{describe(c)}</p>
              <p className="mt-0.5 text-xs text-ink-soft">
                {c.created_at} by {c.actor}
              </p>
            </div>
            <button type="button" className={secondary} onClick={() => back(c.id)} aria-label={`Roll back to change ${c.id}`}>
              Roll back to this
            </button>
          </li>
        ))}
      </ol>
    </section>
  );
};

/** Actions waiting on a student, with deny. */
export const Approvals = () => {
  const { data, error, reload, setError } = useLoad(api.approvals);
  const refuse = (token: string) =>
    api
      .deny(token)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  return (
    <section aria-label="Approvals">
      <Alert text={error} />
      {data?.length === 0 && <p className="text-sm text-ink-soft">Nothing is waiting.</p>}
      <ul className="divide-y divide-ink/10 rounded-md border border-ink/10">
        {data?.map((h) => (
          <li key={h.token} className="flex items-center gap-3 px-4 py-3">
            <div className="flex-1">
              <p className="font-mono text-sm">{h.tool}</p>
              <p className="mt-0.5 text-xs text-ink-soft">
                for {h.user_id}, expires {h.expires_at}
              </p>
            </div>
            <button type="button" className={secondary} onClick={() => refuse(h.token)} aria-label={`Deny ${h.tool} for ${h.user_id}`}>
              Deny
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
};

const LIMIT_NAMES: Record<string, string> = {
  max_turns: "Turns at once",
  model_slots: "Model requests at once (0 is unlimited)",
  rate_limit_per_min: "Requests per user per minute (0 is unlimited)",
  memory_max_per_user: "Memories per user",
  conversation_idle_days: "Days an idle conversation is kept (0 keeps it)",
};

/** The settings that bound load, read only. */
export const Limits = () => {
  const { data, error } = useLoad(api.limits);
  return (
    <section aria-label="Limits">
      <Alert text={error} />
      <dl className="divide-y divide-ink/10 rounded-md border border-ink/10">
        {data &&
          Object.entries(data).map(([key, value]) => (
            <div key={key} className="flex justify-between px-4 py-3 text-sm">
              <dt>{LIMIT_NAMES[key] ?? key}</dt>
              <dd className="font-mono">{value}</dd>
            </div>
          ))}
      </dl>
      <p className="mt-2 text-xs text-ink-soft">These come from sparky.toml and change with a redeploy.</p>
    </section>
  );
};
