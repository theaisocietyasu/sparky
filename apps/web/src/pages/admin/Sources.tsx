import { Database } from "lucide-react";
import * as api from "../../admin/client";
import { useLoad } from "../../admin/useLoad";
import { Alert, Card, Empty, Loading, PageHeader } from "./ui";

/** /admin/sources: the live query sources Platform publishes, read only. */
const Sources = () => {
  const { data, error } = useLoad(api.sources);
  return (
    <section aria-label="Sources">
      <PageHeader title="Sources" description="Live sources Platform search can name. Stored pages are managed on the Platform dashboard." />
      <Alert text={error} />
      <Card className="overflow-x-auto">
        {!data && !error && <Loading />}
        {data?.length === 0 && <Empty icon={Database} title="Platform publishes no live sources." />}
        {data && data.length > 0 && (
          <table className="w-full text-sm">
            <thead className="border-b border-edge text-left text-xs text-dim">
              <tr>
                <th className="px-4 py-2.5 font-medium">Source</th>
                <th className="px-4 py-2.5 font-medium">Parameters</th>
                <th className="px-4 py-2.5 font-medium">Answers</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-edge">
              {data.map((s) => (
                <tr key={s.key}>
                  <td className="px-4 py-3 font-mono">{s.key}</td>
                  <td className="px-4 py-3 text-dim">
                    {s.params.length ? s.params.map((p) => (p.required ? `${p.name}*` : p.name)).join(", ") : "no parameters"}
                  </td>
                  <td className="px-4 py-3 text-dim">{s.description}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </section>
  );
};

export default Sources;
