import * as api from "../../admin/client";
import { useLoad } from "../../admin/useLoad";
import { Alert, Card, Loading, PageHeader } from "./ui";

const NAMES: Record<string, [string, string]> = {
  max_turns: ["Turns at once", "Requests the engine runs at the same time."],
  model_slots: ["Model requests at once", "0 is unlimited."],
  rate_limit_per_min: ["Requests per user per minute", "0 is unlimited."],
  memory_max_per_user: ["Memories per user", ""],
  conversation_idle_days: ["Days an idle conversation is kept", "0 keeps it."],
};

/** /admin/limits: the settings that bound load, read only. */
const Limits = () => {
  const { data, error } = useLoad(api.limits);
  return (
    <section aria-label="Limits">
      <PageHeader title="Limits" description="These come from sparky.toml and change with a redeploy." />
      <Alert text={error} />
      {!data && !error && (
        <Card>
          <Loading rows={3} />
        </Card>
      )}
      {data && (
        <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Object.entries(data).map(([key, value]) => {
            const [name, hint] = NAMES[key] ?? [key, ""];
            return (
              <Card key={key} className="p-4">
                <dt className="text-sm text-dim">{name}</dt>
                <dd className="mt-1 text-2xl font-semibold tabular-nums">{value}</dd>
                {hint && <dd className="mt-1 text-xs text-dim">{hint}</dd>}
              </Card>
            );
          })}
        </dl>
      )}
    </section>
  );
};

export default Limits;
