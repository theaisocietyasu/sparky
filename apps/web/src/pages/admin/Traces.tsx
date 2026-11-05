import { ListChecks } from "lucide-react";
import { Link } from "react-router-dom";
import * as api from "../../admin/client";
import { ms, when } from "../../admin/format";
import { useLoad } from "../../admin/useLoad";
import { Alert, Card, Empty, Loading, PageHeader, StatusBadge } from "./ui";

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
              <StatusBadge status={t.status} />
            </td>
            <td className="hidden px-4 py-3 text-right tabular-nums text-dim sm:table-cell">{t.steps}</td>
            <td className="hidden px-4 py-3 text-right tabular-nums text-dim sm:table-cell">{t.tokens.toLocaleString()}</td>
            <td className="whitespace-nowrap px-4 py-3 text-right tabular-nums text-dim">{ms(t.duration_ms)}</td>
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
