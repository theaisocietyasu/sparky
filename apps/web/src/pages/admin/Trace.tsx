import { ArrowLeft, ChevronRight } from "lucide-react";
import { useCallback } from "react";
import { Link, useParams } from "react-router-dom";
import * as api from "../../admin/client";
import { kindName, summary, when } from "../../admin/format";
import { useLoad } from "../../admin/useLoad";
import { Alert, Card, Loading, PageHeader } from "./ui";

const tone = (kind: string) => {
  if (/error|blocked/.test(kind)) return "bg-bad";
  if (/tool/.test(kind)) return "bg-accent";
  if (kind === "completed") return "bg-good";
  return "bg-dim/60";
};

/** /admin/traces/:id: every recorded event of one request, in order. */
const Trace = () => {
  const { id = "" } = useParams();
  const load = useCallback(() => api.trace(id), [id]);
  const { data, error } = useLoad(load);
  return (
    <section aria-label="Trace">
      <Link to="/admin/traces" className="mb-4 inline-flex items-center gap-1.5 text-sm text-dim hover:text-fg">
        <ArrowLeft className="size-4" />
        Traces
      </Link>
      <PageHeader title="Trace" description={id} />
      <Alert text={error} />
      <Card>
        {!data && !error && <Loading rows={6} />}
        {data && (
          <ol aria-label="Trace events" className="relative py-2">
            {data.map((r, i) => (
              <li key={i} className="relative px-4">
                {i < data.length - 1 && <span aria-hidden className="absolute top-6 bottom-0 left-[1.4rem] w-px bg-edge" />}
                <details className="group">
                  <summary className="flex cursor-pointer list-none items-start gap-3 rounded-lg py-2.5 pr-2 hover:bg-sunk/60">
                    <span className={`relative mt-1.5 ml-1 size-2.5 shrink-0 rounded-full ring-4 ring-raised ${tone(r.kind)}`} />
                    <span className="min-w-0 flex-1">
                      <span className="block text-sm font-medium">{kindName(r.kind)}</span>
                      {summary(r) && <span className="block truncate text-xs text-dim">{summary(r)}</span>}
                    </span>
                    <span className="mt-0.5 shrink-0 text-xs tabular-nums text-dim">{when(r.at)}</span>
                    <ChevronRight className="mt-1 size-4 shrink-0 text-dim transition-transform group-open:rotate-90" />
                  </summary>
                  <pre className="mb-2 ml-8 overflow-x-auto rounded-lg border border-edge bg-sunk p-3 font-mono text-xs leading-relaxed">
                    {JSON.stringify(r, null, 2)}
                  </pre>
                </details>
              </li>
            ))}
          </ol>
        )}
      </Card>
    </section>
  );
};

export default Trace;
