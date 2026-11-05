import { useCallback, useState } from "react";

/** The two app themes. */
export type Theme = "light" | "dark";

const KEY = "sparky-theme";

const current = (): Theme => (document.documentElement.dataset.theme === "dark" ? "dark" : "light");

/** The app theme, a setter and a toggle; the choice stays in this browser. */
export const useTheme = () => {
  const [theme, setTheme] = useState<Theme>(current);
  const set = useCallback((next: Theme) => {
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // Storage is off in this browser; the choice lasts for this page only.
    }
    setTheme(next);
  }, []);
  const toggle = useCallback(() => set(current() === "dark" ? "light" : "dark"), [set]);
  return { theme, set, toggle };
};
