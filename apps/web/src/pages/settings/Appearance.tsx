import { Check, Moon, Sun } from "lucide-react";
import { type Theme, useTheme } from "../../app/theme";

const THEMES: [Theme, string, typeof Sun][] = [
  ["light", "Light", Sun],
  ["dark", "Dark", Moon],
];

/** Settings, Appearance: the app theme. */
const Appearance = () => {
  const { theme, set } = useTheme();
  return (
    <section aria-labelledby="settings-title">
      <h2 id="settings-title" className="text-lg font-semibold tracking-tight">
        Appearance
      </h2>
      <div role="radiogroup" aria-label="Theme" className="mt-6 grid gap-3 sm:grid-cols-2">
        {THEMES.map(([key, label, Icon]) => (
          <button
            key={key}
            type="button"
            role="radio"
            aria-checked={theme === key}
            onClick={() => set(key)}
            className={`glass flex h-16 items-center gap-3 rounded-2xl px-4 text-left transition ${theme === key ? "ring-2 ring-accent" : "hover:bg-sunk/40"}`}
          >
            <Icon className="size-5 text-dim" />
            <span className="flex-1 text-sm font-medium">{label}</span>
            {theme === key && <Check className="size-4 text-accent" />}
          </button>
        ))}
      </div>
    </section>
  );
};

export default Appearance;
