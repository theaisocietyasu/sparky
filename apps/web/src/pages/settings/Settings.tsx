import { Bell, Brain, Link2, Palette, UserRound, type LucideIcon } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { Navigate, NavLink, useParams } from "react-router-dom";
import ChatFrame from "../../app/ChatFrame";
import Accounts from "./Accounts";
import Appearance from "./Appearance";
import Memories from "./Memories";
import Profile from "./Profile";
import Notifications from "./Notifications";

const SECTIONS: { key: string; label: string; icon: LucideIcon; page: () => React.JSX.Element }[] = [
  { key: "profile", label: "Profile", icon: UserRound, page: Profile },
  { key: "accounts", label: "Connected accounts", icon: Link2, page: Accounts },
  { key: "notifications", label: "Notifications", icon: Bell, page: Notifications },
  { key: "memories", label: "Memories", icon: Brain, page: Memories },
  { key: "appearance", label: "Appearance", icon: Palette, page: Appearance },
];

/** /settings/:section: the student's profile, linked accounts, notifications, memories and theme. */
const Settings = () => {
  const { section = "profile" } = useParams();
  const still = useReducedMotion() ?? false;
  const shown = SECTIONS.find((s) => s.key === section);
  if (!shown) return <Navigate to="/settings" replace />;
  const Page = shown.page;
  return (
    <ChatFrame>
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-4xl px-4 pb-10 pt-24">
          <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
          <div className="mt-6 flex flex-col gap-8 md:flex-row">
            <nav aria-label="Settings" className="-mx-1 flex gap-1 overflow-x-auto md:mx-0 md:w-52 md:shrink-0 md:flex-col">
              {SECTIONS.map((s) => (
                <NavLink
                  key={s.key}
                  to={s.key === "profile" ? "/settings" : `/settings/${s.key}`}
                  end
                  className={({ isActive }) =>
                    `flex h-9 shrink-0 items-center gap-2.5 rounded-lg px-2.5 text-sm transition-colors ${
                      isActive ? "bg-sunk font-medium text-fg" : "text-dim hover:bg-sunk/70 hover:text-fg"
                    }`
                  }
                >
                  <s.icon className="size-4 shrink-0" />
                  {s.label}
                </NavLink>
              ))}
            </nav>
            <motion.div
              key={shown.key}
              initial={still ? false : { opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className="min-w-0 flex-1"
            >
              <Page />
            </motion.div>
          </div>
        </div>
      </main>
    </ChatFrame>
  );
};

export default Settings;
