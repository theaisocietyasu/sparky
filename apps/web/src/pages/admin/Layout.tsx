import { ArrowLeft, Database, Gauge, History, LayoutDashboard, ListChecks, ScrollText, ShieldAlert, Wrench } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { Suspense } from "react";
import { Link, Outlet, useLocation } from "react-router-dom";
import Frame, { type NavGroup } from "../../app/Frame";
import { isModerator, useMe } from "../../app/useMe";
import { Empty } from "./ui";

const GROUPS: NavGroup[] = [
  { items: [{ to: "/admin", label: "Overview", icon: LayoutDashboard, end: true }] },
  {
    title: "Configure",
    items: [
      { to: "/admin/tools", label: "Tools", icon: Wrench },
      { to: "/admin/instructions", label: "Instructions", icon: ScrollText },
    ],
  },
  {
    title: "Monitor",
    items: [
      { to: "/admin/traces", label: "Traces", icon: ListChecks },
      { to: "/admin/approvals", label: "Approvals", icon: ShieldAlert },
      { to: "/admin/history", label: "History", icon: History },
    ],
  },
  {
    title: "Reference",
    items: [
      { to: "/admin/sources", label: "Sources", icon: Database },
      { to: "/admin/limits", label: "Limits", icon: Gauge },
    ],
  },
];

const back = (
  <Link to="/chat" className="flex h-9 w-full items-center gap-2.5 rounded-lg px-2 text-sm text-dim hover:bg-sunk hover:text-fg">
    <ArrowLeft className="size-4" />
    Back to chat
  </Link>
);

/** Layout route of /admin: the admin sidebar, and the page with a short fade on each route change. */
const AdminLayout = () => {
  const { me } = useMe();
  const { pathname } = useLocation();
  const still = useReducedMotion() ?? false;
  return (
    <Frame label="Admin" badge="Admin" actions={back} groups={GROUPS}>
      <main className="flex-1 overflow-y-auto">
        <motion.div
          key={pathname}
          initial={still ? false : { opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-8"
        >
          {isModerator(me) ? (
            <Suspense fallback={null}>
              <Outlet />
            </Suspense>
          ) : (
            <div role="alert">
              <Empty icon={ShieldAlert} title="This needs the Manage Server role in the Discord server." hint="Ask a server manager for the role, then sign in again." />
            </div>
          )}
        </motion.div>
      </main>
    </Frame>
  );
};

export default AdminLayout;
