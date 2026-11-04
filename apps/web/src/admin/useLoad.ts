import { useCallback, useEffect, useState } from "react";
import { EngineError } from "../chat/client";

/** Loads data once and on reload, with the refusal as a message. */
export const useLoad = <T,>(load: () => Promise<T>) => {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    load()
      .then((d) => {
        setData(d);
        setError(null);
      })
      .catch((e: unknown) => setError(refusal(e)));
  }, [load]);

  useEffect(reload, [reload]);
  return { data, error, reload, setError };
};

/** What a failed admin call tells the moderator. */
export const refusal = (e: unknown) => {
  if (e instanceof EngineError) {
    if (e.status === 403) return "This needs the Manage Server role in the Discord server.";
    if (e.status === 401) return "Your session ended. Sign in again.";
    if (e.status === 501) return "This engine keeps no admin store.";
    return e.message;
  }
  return "Sparky is not reachable right now.";
};
