import { ExternalLink, ListChecks, ShieldX } from "lucide-react";
import { Link } from "react-router-dom";
import * as api from "../../admin/client";
import { ms, when } from "../../admin/format";
import { useLoad } from "../../admin/useLoad";
import { Alert, Card, Empty, Loading, PageHeader, StatusBadge } from "./ui";

/** What the guardrail did on a request, for the badge tooltip. */
const guarded = (t: api.TraceSummary) =>
  [
    t.guardrail_blocks ? `${t.guardrail_blocks} blocked` : "",
    t.guardrail_redactions ? `${t.guardrail_redactions} redacted` : "",
  ]
    .filter(Boolean)
    .join(", ") + " by the guardrail";

/** The trace table, shared by the Traces page and the overview. */
export const TraceTable = ({ rows }: { rows: api.TraceSummary[] }) => (
  <div className="overflow-x-auto">
    <table className="w-full text-sm">
      <thead className="border-b border-edge text-left text-xs text-dim">
        <tr>
          <th className="px-4 py-2.5 font-medium">Request</th>
          <th className="px-4 py-2.5 font-medium">Status</th>
          <th className="hidden px-4 py-2.5 text-right font-medium sm:table-cell">Steps</th>
          <th className="hidden px-4 py-2.5 text-right font-medium sm:table-cell">Tokens</th>
          <th className="px-4 py-2.5 text-right font-medium">Time</th>
          <th className="w-10 py-2.5 pr-3">
            <span className="sr-only">Phoenix</span>
          </th>
        </tr>
      </thead>
      <tbody className="divide-y divide-edge">
        {rows.map((t) => (
          <tr key={t.request_id} className="hover:bg-sunk/60">
            <td className="w-full max-w-0 px-4 py-3">
              <Link to={`/admin/traces/${encodeURIComponent(t.request_id)}`} aria-label={`Open trace ${t.request_id}`} className="block">
                <span className="block truncate font-medium">{t.input || "(no text)"}</span>
                <span className="block truncate text-xs text-dim">
                  {when(t.at)} by {t.user_id}
                </span>
              </Link>
            </td>
            <td className="px-4 py-3">
              <span className="flex items-center gap-1.5">
                <StatusBadge status={t.status} />
                {(t.guardrail_blocks ?? 0) + (t.guardrail_redactions ?? 0) > 0 && (
                  <span title={guarded(t)} aria-label={guarded(t)} className="text-warn">
                    <ShieldX className="size-3.5" />
                  </span>
                )}
              </span>
            </td>
            <td className="hidden px-4 py-3 text-right tabular-nums text-dim sm:table-cell">{t.steps}</td>
            <td className="hidden px-4 py-3 text-right tabular-nums text-dim sm:table-cell">{t.tokens.toLocaleString()}</td>
            <td className="whitespace-nowrap px-4 py-3 text-right tabular-nums text-dim">{ms(t.duration_ms)}</td>
            <td className="py-3 pr-3">
              {t.phoenix_url && (
                <a
                  href={t.phoenix_url}
                  target="_blank"
                  rel="noreferrer"
                  aria-label={`Open trace ${t.request_id} in Phoenix`}
                  title="Open in Phoenix"
                  className="grid size-7 place-items-center rounded-md text-dim hover:bg-sunk hover:text-fg"
                >
                  <ExternalLink className="size-3.5" />
                </a>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
);

/** /admin/traces: the newest requests; each opens to its own page. */
const Traces = () => {
  const { data, error } = useLoad(api.traces);
  return (
    <section aria-label="Traces">
      <PageHeader title="Traces" description="The newest requests and what the agent did on each. Linked-account results are masked." />
      <Alert text={error} />
      <Card>
        {!data && !error && <Loading rows={6} />}
        {data?.length === 0 && <Empty icon={ListChecks} title="No traces yet." hint="A trace appears after the first question." />}
        {data && data.length > 0 && <TraceTable rows={data} />}
      </Card>
    </section>
  );
};

export default Traces;
