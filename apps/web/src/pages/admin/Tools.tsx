import { Search, Wrench } from "lucide-react";
import { useState } from "react";
import * as api from "../../admin/client";
import { refusal, useLoad } from "../../admin/useLoad";
import { Alert, Badge, Card, Empty, Loading, PageHeader, Switch } from "./ui";

/** /admin/tools: every tool with an on and off switch, filtered by name or description. */
const Tools = () => {
  const { data, error, reload, setError } = useLoad(api.tools);
  const [query, setQuery] = useState("");
  const flip = (name: string, on: boolean) =>
    api
      .switchTool(name, on)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  const q = query.trim().toLowerCase();
  const shown = data?.filter((t) => !q || t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q));
  const on = data?.filter((t) => t.on).length ?? 0;
  return (
    <section aria-label="Tools">
      <PageHeader title="Tools" description={data ? `${on} of ${data.length} tools are offered to the model.` : "What the model may call."} />
      <Alert text={error} />
      <Card>
        <div className="flex items-center gap-2 border-b border-edge px-4">
          <Search className="size-4 text-dim" />
          <label htmlFor="tool-search" className="sr-only">
            Filter tools
          </label>
          <input
            id="tool-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter by name or description"
            className="h-11 flex-1 bg-transparent text-sm outline-none placeholder:text-dim"
          />
        </div>
        {!data && !error && <Loading />}
        {shown?.length === 0 && <Empty icon={Wrench} title="No tool matches that filter." />}
        <ul className="divide-y divide-edge">
          {shown?.map((t) => (
            <li key={t.name} className="flex items-start gap-4 px-4 py-3.5">
              <div className="min-w-0 flex-1">
                <p className="flex flex-wrap items-center gap-2">
                  <span className="font-mono text-sm">{t.name}</span>
                  {t.private && <Badge>direct messages only</Badge>}
                </p>
                <p className="mt-1 line-clamp-2 text-sm text-dim">{t.description}</p>
              </div>
              <Switch on={t.on} onChange={(v) => flip(t.name, v)} label={`Offer ${t.name}`} />
            </li>
          ))}
        </ul>
      </Card>
    </section>
  );
};

export default Tools;
