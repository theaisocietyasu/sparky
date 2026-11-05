import { ChevronsUpDown, LayoutDashboard, LogOut, Menu, Moon, Settings, Sun, X, type LucideIcon } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { type MouseEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { Link, NavLink } from "react-router-dom";
import Sparky from "../components/sparky/Sparky";
import { isModerator, useMe } from "./useMe";
import { useTheme } from "./theme";

/** One sidebar link. */
export type NavItem = { to: string; label: string; icon: LucideIcon; end?: boolean };

/** A titled group of sidebar links; a group with no title has no heading. */
export type NavGroup = { title?: string; items: NavItem[] };

/** The profile button at the foot of the sidebar and its menu: settings, admin, theme and sign out. */
const Account = () => {
  const { me, signOut } = useMe();
  const { theme, toggle } = useTheme();
  const [open, setOpen] = useState(false);
  const still = useReducedMotion() ?? false;
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const item = "flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-sm text-fg hover:bg-sunk";
  return (
    <div ref={box} className="relative p-3">
      <AnimatePresence>
        {open && (
          <motion.div
            role="menu"
            aria-label="Account"
            initial={still ? false : { opacity: 0, y: 6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={still ? undefined : { opacity: 0, y: 4, scale: 0.98 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="glass absolute inset-x-3 bottom-full mb-1 origin-bottom rounded-xl p-1.5"
          >
            <Link role="menuitem" to="/settings" onClick={() => setOpen(false)} className={item}>
              <Settings className="size-4 text-dim" />
              Settings
            </Link>
            {isModerator(me) && (
              <Link role="menuitem" to="/admin" onClick={() => setOpen(false)} className={item}>
                <LayoutDashboard className="size-4 text-dim" />
                Admin
              </Link>
            )}
            <button role="menuitem" type="button" onClick={toggle} className={item}>
              {theme === "dark" ? <Sun className="size-4 text-dim" /> : <Moon className="size-4 text-dim" />}
              {theme === "dark" ? "Light theme" : "Dark theme"}
            </button>
            <div className="my-1 border-t border-edge" />
            <button role="menuitem" type="button" onClick={signOut} className={item}>
              <LogOut className="size-4 text-dim" />
              Sign out
            </button>
          </motion.div>
        )}
      </AnimatePresence>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="Account menu"
        className="flex h-11 w-full items-center gap-2.5 rounded-xl px-2 text-left transition-colors hover:bg-sunk/70"
      >
        <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-full bg-accent text-sm font-semibold text-accent-fg">
          {me.name.slice(0, 1).toUpperCase()}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm font-medium">{me.name}</span>
        <ChevronsUpDown className="size-4 shrink-0 text-dim" />
      </button>
    </div>
  );
};

const Links = ({ groups, label, children }: { groups: NavGroup[]; label: string; children?: ReactNode }) => (
  <nav aria-label={label} className="flex-1 overflow-y-auto px-3 py-2">
    {groups.map((g, i) => (
      <div key={g.title ?? i} className="mb-4">
        {g.title && <p className="px-2 pb-1 text-xs font-medium text-dim">{g.title}</p>}
        <ul className="space-y-0.5">
          {g.items.map((item) => (
            <li key={item.to}>
              <NavLink
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  `flex h-9 items-center gap-2.5 rounded-lg px-2 text-sm transition-colors ${
                    isActive ? "bg-sunk font-medium text-fg" : "text-dim hover:bg-sunk/70 hover:text-fg"
                  }`
                }
              >
                <item.icon className="size-4 shrink-0" />
                {item.label}
              </NavLink>
            </li>
          ))}
        </ul>
      </div>
    ))}
    {children}
  </nav>
);

/** The app page frame: a sidebar with the brand, actions, links and extra content, and the page beside it. */
const Frame = ({
  label,
  badge,
  actions,
  groups,
  extra,
  children,
}: {
  label: string;
  badge?: string;
  actions?: ReactNode;
  groups: NavGroup[];
  extra?: ReactNode;
  children: ReactNode;
}) => {
  const [open, setOpen] = useState(false);
  const closeOnLink = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest("a")) setOpen(false);
  };

  const brand = (
    <Link to="/" className="flex items-center gap-2 text-[15px] font-semibold tracking-tight">
      <Sparky className="h-6 w-auto" />
      Sparky
      {badge && <span className="rounded-md bg-sunk px-1.5 py-0.5 text-xs font-medium text-dim">{badge}</span>}
    </Link>
  );

  return (
    <div className="aura flex h-dvh text-fg">
      {open && <button type="button" aria-label="Close the menu" onClick={() => setOpen(false)} className="fixed inset-0 z-30 bg-black/40 md:hidden" />}
      <aside
        onClick={closeOnLink}
        className={`fixed inset-y-0 left-0 z-40 flex w-64 flex-col border-r border-glass-edge bg-side/80 backdrop-blur-xl transition-transform md:static md:translate-x-0 ${
          open ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        <div className="flex h-14 items-center justify-between px-4">
          {brand}
          <button type="button" aria-label="Close the menu" onClick={() => setOpen(false)} className="text-dim md:hidden">
            <X className="size-5" />
          </button>
        </div>
        {actions && <div className="px-3 pb-2">{actions}</div>}
        <Links groups={groups} label={label}>
          {extra}
        </Links>
        <Account />
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center gap-3 border-b border-glass-edge px-4 md:hidden">
          <button type="button" aria-label="Open the menu" onClick={() => setOpen(true)} className="text-dim">
            <Menu className="size-5" />
          </button>
          {brand}
        </header>
        {children}
      </div>
    </div>
  );
};

export default Frame;
