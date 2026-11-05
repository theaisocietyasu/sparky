import { ArrowRight, ListChecks } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import * as api from "../../admin/client";
import { describe, figures, ms, when } from "../../admin/format";
import { useLoad } from "../../admin/useLoad";
import { TraceTable } from "./Traces";
import { Alert, Card, Empty, Loading, PageHeader } from "./ui";

const Stat = ({ label, value, hint, to }: { label: string; value: ReactNode; hint?: string; to?: string }) => {
  const body = (
    <>
      <p className="text-sm text-dim">{label}</p>
      <p className="mt-1.5 text-2xl font-semibold tabular-nums tracking-tight">{value}</p>
      {hint && <p className="mt-1 text-xs text-dim">{hint}</p>}
    </>
  );
  return to ? (
    <Link to={to} className="block rounded-xl border border-edge bg-raised p-4 transition-colors hover:border-dim">
      {body}
    </Link>
  ) : (
    <Card className="p-4">{body}</Card>
  );
};

const Section = ({ title, to, children }: { title: string; to: string; children: ReactNode }) => (
  <Card>
    <div className="flex items-center justify-between border-b border-edge px-4 py-3">
      <h2 className="text-sm font-semibold">{title}</h2>
      <Link to={to} className="inline-flex items-center gap-1 text-xs text-dim hover:text-fg">
        View all
        <ArrowRight className="size-3.5" />
      </Link>
    </div>
    {children}
  </Card>
);

/** /admin: the state of Sparky at a glance, from the newest traces, the tools, approvals and changes. */
const Overview = () => {
  const traces = useLoad(api.traces);
  const tools = useLoad(api.tools);
  const approvals = useLoad(api.approvals);
  const history = useLoad(api.history);
  const f = traces.data ? figures(traces.data) : null;
  const dash = "-";
  return (
    <section aria-label="Overview">
      <PageHeader title="Overview" description="Figures cover the newest requests the engine keeps traces for." />
      <Alert text={traces.error ?? tools.error ?? approvals.error ?? history.error} />
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Stat label="Requests" value={f ? f.requests : dash} hint={f ? `from ${f.users} ${f.users === 1 ? "person" : "people"}` : undefined} to="/admin/traces" />
        <Stat label="Answered" value={f?.answeredShare != null ? `${f.answeredShare}%` : dash} hint="of finished requests" />
        <Stat label="Median time" value={f?.median != null ? ms(f.median) : dash} hint={f ? `${f.tokens.toLocaleString()} tokens in all` : undefined} />
        <Stat
          label="Tools on"
          value={tools.data ? `${tools.data.filter((t) => t.on).length} / ${tools.data.length}` : dash}
          hint={approvals.data ? `${approvals.data.length} waiting for approval` : undefined}
          to="/admin/tools"
        />
      </div>
      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <Section title="Recent requests" to="/admin/traces">
            {!traces.data && !traces.error && <Loading />}
            {traces.data?.length === 0 && <Empty icon={ListChecks} title="No traces yet." />}
            {traces.data && traces.data.length > 0 && <TraceTable rows={traces.data.slice(0, 6)} />}
          </Section>
        </div>
        <Section title="Recent changes" to="/admin/history">
          {!history.data && !history.error && <Loading rows={3} />}
          {history.data?.length === 0 && <p className="px-4 py-6 text-sm text-dim">No changes yet.</p>}
          <ol className="divide-y divide-edge">
            {history.data?.slice(0, 6).map((c) => (
              <li key={c.id} className="px-4 py-3">
                <p className="truncate text-sm">{describe(c)}</p>
                <p className="mt-0.5 text-xs text-dim">
                  {when(c.created_at)} by {c.actor}
                </p>
              </li>
            ))}
          </ol>
        </Section>
      </div>
    </section>
  );
};

export default Overview;
