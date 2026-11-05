import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { status as statusOf, type Tone } from "../../admin/format";

/** The title row of an admin page, with its actions on the right. */
export const PageHeader = ({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) => (
  <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
    <div>
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      {description && <p className="mt-1 text-sm text-dim">{description}</p>}
    </div>
    {actions && <div className="flex gap-2">{actions}</div>}
  </div>
);

/** A bordered surface. */
export const Card = ({ children, className = "" }: { children: ReactNode; className?: string }) => (
  <div className={`rounded-xl border border-edge bg-raised ${className}`}>{children}</div>
);

/** A refusal or failure, shown above the page content. */
export const Alert = ({ text }: { text: string | null }) =>
  text ? (
    <p role="alert" className="mb-4 rounded-lg border border-bad/30 bg-bad/10 px-3 py-2 text-sm text-bad">
      {text}
    </p>
  ) : null;

/** What an empty list says, with an icon. */
export const Empty = ({ icon: Icon, title, hint }: { icon: LucideIcon; title: string; hint?: string }) => (
  <div className="flex flex-col items-center px-6 py-14 text-center">
    <Icon className="size-6 text-dim" />
    <p className="mt-3 text-sm font-medium">{title}</p>
    {hint && <p className="mt-1 text-sm text-dim">{hint}</p>}
  </div>
);

/** Grey bars in place of rows while a list loads. */
export const Loading = ({ rows = 4 }: { rows?: number }) => (
  <div aria-hidden className="space-y-2 p-4">
    {Array.from({ length: rows }, (_, i) => (
      <div key={i} className="h-10 animate-pulse rounded-lg bg-sunk" />
    ))}
  </div>
);

const TONES: Record<Tone, string> = {
  good: "bg-good/10 text-good",
  warn: "bg-warn/10 text-warn",
  bad: "bg-bad/10 text-bad",
  plain: "bg-sunk text-dim",
};

/** A small label with a tone. */
export const Badge = ({ tone = "plain", children }: { tone?: Tone; children: ReactNode }) => (
  <span className={`inline-flex h-5 items-center rounded-md px-1.5 text-xs font-medium ${TONES[tone]}`}>{children}</span>
);

/** How a request ended, as a badge. */
export const StatusBadge = ({ status }: { status: string | null }) => {
  const [tone, label] = statusOf(status);
  return <Badge tone={tone}>{label}</Badge>;
};

/** An on and off switch. */
export const Switch = ({ on, onChange, label }: { on: boolean; onChange: (on: boolean) => void; label: string }) => (
  <button
    type="button"
    role="switch"
    aria-checked={on}
    aria-label={label}
    onClick={() => onChange(!on)}
    className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${on ? "bg-good" : "bg-edge"}`}
  >
    <span className={`absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow-sm transition-transform ${on ? "translate-x-4" : ""}`} />
  </button>
);

