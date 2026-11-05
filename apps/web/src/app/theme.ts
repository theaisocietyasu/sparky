import { useCallback, useState } from "react";

/** The two app themes. */
export type Theme = "light" | "dark";

const KEY = "sparky-theme";

const current = (): Theme => (document.documentElement.dataset.theme === "dark" ? "dark" : "light");

/** The app theme and a toggle that keeps the choice in this browser. */
export const useTheme = () => {
  const [theme, setTheme] = useState<Theme>(current);
  const toggle = useCallback(() => {
    const next: Theme = current() === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // Storage is off in this browser; the choice lasts for this page only.
    }
    setTheme(next);
  }, []);
  return { theme, toggle };
};
