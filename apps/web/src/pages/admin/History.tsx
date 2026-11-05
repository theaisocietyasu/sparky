import { History as HistoryIcon, RotateCcw, ScrollText, Wrench } from "lucide-react";
import * as api from "../../admin/client";
import { button, describe, when } from "../../admin/format";
import { refusal, useLoad } from "../../admin/useLoad";
import { Alert, Card, Empty, Loading, PageHeader } from "./ui";

/** /admin/history: every admin change, newest first, with rollback. */
const History = () => {
  const { data, error, reload, setError } = useLoad(api.history);
  const back = (id: number) =>
    api
      .rollback(id)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  return (
    <section aria-label="History">
      <PageHeader title="History" description="Every change a moderator made. Roll back to any of them." />
      <Alert text={error} />
      <Card>
        {!data && !error && <Loading />}
        {data?.length === 0 && <Empty icon={HistoryIcon} title="No changes yet." />}
        <ol className="divide-y divide-edge">
          {data?.map((c) => {
            const Icon = c.kind === "tool" ? Wrench : ScrollText;
            return (
              <li key={c.id} className="flex items-center gap-4 px-4 py-3.5">
                <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-sunk text-dim">
                  <Icon className="size-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm">{describe(c)}</p>
                  <p className="mt-0.5 text-xs text-dim">
                    {when(c.created_at)} by {c.actor}
                  </p>
                </div>
                <button type="button" className={button.secondary} onClick={() => back(c.id)} aria-label={`Roll back to change ${c.id}`}>
                  <RotateCcw className="size-3.5" />
                  Roll back
                </button>
              </li>
            );
          })}
        </ol>
      </Card>
    </section>
  );
};

export default History;
