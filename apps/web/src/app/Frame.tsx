import { LogOut, Menu, Moon, Sun, X, type LucideIcon } from "lucide-react";
import { type MouseEvent, type ReactNode, useState } from "react";
import { Link, NavLink } from "react-router-dom";
import { useMe } from "./useMe";
import { useTheme } from "./theme";

/** One sidebar link. */
export type NavItem = { to: string; label: string; icon: LucideIcon; end?: boolean };

/** A titled group of sidebar links; a group with no title has no heading. */
export type NavGroup = { title?: string; items: NavItem[] };

const Account = () => {
  const { me, signOut } = useMe();
  const { theme, toggle } = useTheme();
  return (
    <div className="flex items-center gap-2 border-t border-edge p-3">
      <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-full bg-accent text-sm font-semibold text-accent-fg">
        {me.name.slice(0, 1).toUpperCase()}
      </span>
      <span className="min-w-0 flex-1 truncate text-sm font-medium">{me.name}</span>
      <button
        type="button"
        onClick={toggle}
        aria-label={theme === "dark" ? "Use the light theme" : "Use the dark theme"}
        className="grid size-8 place-items-center rounded-lg text-dim hover:bg-sunk hover:text-fg"
      >
        {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
      </button>
      <button
        type="button"
        onClick={signOut}
        aria-label="Sign out"
        className="grid size-8 place-items-center rounded-lg text-dim hover:bg-sunk hover:text-fg"
      >
        <LogOut className="size-4" />
      </button>
    </div>
  );
};

const Links = ({ groups, label }: { groups: NavGroup[]; label: string }) => (
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
  </nav>
);

/** The app page frame: a sidebar with the brand, actions and links, and the page beside it. */
const Frame = ({
  label,
  badge,
  actions,
  groups,
  children,
}: {
  label: string;
  badge?: string;
  actions?: ReactNode;
  groups: NavGroup[];
  children: ReactNode;
}) => {
  const [open, setOpen] = useState(false);
  const closeOnLink = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest("a")) setOpen(false);
  };

  const brand = (
    <Link to="/" className="flex items-center gap-2 text-[15px] font-semibold tracking-tight">
      <img src="/brand/sparkyai-logo.png" alt="" width={717} height={779} className="h-6 w-auto" />
      Sparky
      {badge && <span className="rounded-md bg-sunk px-1.5 py-0.5 text-xs font-medium text-dim">{badge}</span>}
    </Link>
  );

  return (
    <div className="flex h-dvh bg-canvas text-fg">
      {open && <button type="button" aria-label="Close the menu" onClick={() => setOpen(false)} className="fixed inset-0 z-30 bg-black/40 md:hidden" />}
      <aside
        onClick={closeOnLink}
        className={`fixed inset-y-0 left-0 z-40 flex w-64 flex-col border-r border-edge bg-side transition-transform md:static md:translate-x-0 ${
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
        <Links groups={groups} label={label} />
        <Account />
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center gap-3 border-b border-edge px-4 md:hidden">
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
