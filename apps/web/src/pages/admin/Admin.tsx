import { NavLink, useParams } from "react-router-dom";
import Gated from "../../components/chat/Gated";
import { Approvals, History, Instructions, Limits, Sources, Tools, Traces } from "./Tabs";

const TABS = [
  { path: "tools", label: "Tools", view: Tools },
  { path: "instructions", label: "Instructions", view: Instructions },
  { path: "history", label: "History", view: History },
  { path: "approvals", label: "Approvals", view: Approvals },
  { path: "traces", label: "Traces", view: Traces },
  { path: "sources", label: "Sources", view: Sources },
  { path: "limits", label: "Limits", view: Limits },
] as const;

const Panel = () => {
  const { tab = "tools" } = useParams();
  const current = TABS.find((t) => t.path === tab) ?? TABS[0];
  const View = current.view;
  return (
    <>
      <h1 className="font-serif text-2xl font-bold">Admin</h1>
      <nav aria-label="Admin" className="mt-4 flex gap-4 border-b border-ink/10 text-sm">
        {TABS.map((t) => (
          <NavLink
            key={t.path}
            to={`/admin/${t.path}`}
            className={() =>
              `-mb-px border-b-2 pb-2 ${t.path === current.path ? "border-shu text-ink" : "border-transparent text-ink-soft hover:text-ink"}`
            }
          >
            {t.label}
          </NavLink>
        ))}
      </nav>
      <div className="mt-4">
        <View />
      </div>
    </>
  );
};

/** /admin: tools, instructions, history, approvals, traces, sources and limits for a server manager. */
const Admin = () => <Gated>{() => <Panel />}</Gated>;

export default Admin;
