import { ShieldCheck } from "lucide-react";
import * as api from "../../admin/client";
import { button, when } from "../../admin/format";
import { refusal, useLoad } from "../../admin/useLoad";
import { Alert, Card, Empty, Loading, PageHeader } from "./ui";

/** /admin/approvals: actions waiting on a student, with deny. */
const Approvals = () => {
  const { data, error, reload, setError } = useLoad(api.approvals);
  const refuse = (token: string) =>
    api
      .deny(token)
      .then(reload)
      .catch((e: unknown) => setError(refusal(e)));
  return (
    <section aria-label="Approvals">
      <PageHeader title="Approvals" description="Actions Sparky holds until the student approves them. Deny one to cancel it." />
      <Alert text={error} />
      <Card>
        {!data && !error && <Loading rows={3} />}
        {data?.length === 0 && <Empty icon={ShieldCheck} title="Nothing is waiting." />}
        <ul className="divide-y divide-edge">
          {data?.map((h) => (
            <li key={h.token} className="flex items-center gap-4 px-4 py-3.5">
              <div className="min-w-0 flex-1">
                <p className="font-mono text-sm">{h.tool}</p>
                <p className="mt-0.5 text-xs text-dim">
                  for {h.user_id}, expires {when(h.expires_at)}
                </p>
              </div>
              <button type="button" className={button.danger} onClick={() => refuse(h.token)} aria-label={`Deny ${h.tool} for ${h.user_id}`}>
                Deny
              </button>
            </li>
          ))}
        </ul>
      </Card>
    </section>
  );
};

export default Approvals;
