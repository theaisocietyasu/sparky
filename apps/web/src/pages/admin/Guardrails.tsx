import { ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import * as api from "../../admin/client";
import { useLoad } from "../../admin/useLoad";
import { TraceTable } from "./Traces";
import { Alert, Badge, Card, Empty, Loading, PageHeader } from "./ui";

/** A list of terms as chips, or a dash when it is empty. */
const Terms = ({ items, empty }: { items: string[]; empty: string }) =>
  items.length ? (
    <ul className="flex flex-wrap gap-1.5">
      {items.map((t) => (
        <li key={t} className="rounded-md bg-sunk px-2 py-0.5 font-mono text-xs">
          {t}
        </li>
      ))}
    </ul>
  ) : (
    <p className="text-sm text-dim">{empty}</p>
  );

/** One labelled setting. */
const Row = ({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) => (
  <div className="grid gap-1.5 px-4 py-3.5 sm:grid-cols-[14rem_1fr] sm:gap-4">
    <dt>
      <p className="text-sm font-medium">{label}</p>
      {hint && <p className="text-xs text-dim">{hint}</p>}
    </dt>
    <dd className="min-w-0 text-sm">{children}</dd>
  </div>
);

const yes = (on: boolean, good = true) => <Badge tone={on === good ? "good" : "warn"}>{on ? "On" : "Off"}</Badge>;

/** /admin/guardrails: what Sparky refuses or hides, what needs a moderator, and recent requests it stopped. */
const Guardrails = () => {
  const { data, error } = useLoad(api.guardrails);
  const recent = useLoad(api.traces);
  const stopped = recent.data?.filter((t) => (t.guardrail_blocks ?? 0) + (t.guardrail_redactions ?? 0) > 0) ?? [];

  return (
    <section aria-label="Guardrails">
      <PageHeader title="Guardrails" description="These come from sparky.toml and change with a redeploy." />
      <Alert text={error} />
      {!data && !error && (
        <Card>
          <Loading rows={5} />
        </Card>
      )}
      {data && (
        <div className="space-y-6">
          <Card>
            <h2 className="border-b border-edge px-4 py-3 text-sm font-semibold">Answers</h2>
            <dl className="divide-y divide-edge">
              <Row label="Guardrail" hint="Checks every response before it is shown.">
                {yes(data.enabled)}
              </Row>
              <Row label="Blocked phrases" hint="A response with one is replaced.">
                <Terms items={data.denied_phrases} empty="None" />
              </Row>
              <Row label="Hidden terms" hint={`Shown as ${data.redaction}.`}>
                <Terms items={data.protected_terms} empty="None" />
              </Row>
              <Row label="Longest answer">
                <span className="tabular-nums">{data.max_answer_chars ? `${data.max_answer_chars.toLocaleString()} characters` : "No limit"}</span>
              </Row>
              <Row label="Shown when blocked">
                <p className="text-dim">{data.replacement}</p>
              </Row>
            </dl>
          </Card>
          <Card>
            <h2 className="border-b border-edge px-4 py-3 text-sm font-semibold">Actions</h2>
            <dl className="divide-y divide-edge">
              <Row label="Roles that can write" hint="Everyone else is refused.">
                <Terms items={data.write_roles} empty="Nobody" />
              </Row>
              <Row label="Needs confirmation from" hint="This risk and above waits for the student.">
                <span className="font-mono text-xs">{data.confirm_from}</span>
              </Row>
              <Row label="Linked-account reads" hint="Tools read inside a student's own account.">
                {yes(data.allow_authenticated_reads, false)}
              </Row>
            </dl>
          </Card>
          <Card>
            <h2 className="border-b border-edge px-4 py-3 text-sm font-semibold">Recent blocks</h2>
            {!recent.data && !recent.error && <Loading rows={2} />}
            {recent.data && stopped.length === 0 && (
              <Empty icon={ShieldCheck} title="Nothing blocked lately." hint="Requests the guardrail stopped show here." />
            )}
            {stopped.length > 0 && <TraceTable rows={stopped} />}
          </Card>
        </div>
      )}
    </section>
  );
};

export default Guardrails;
