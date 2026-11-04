import { useCallback, useEffect, useState } from "react";
import { engineUrl, logout, me as fetchMe } from "./client";
import type { Me } from "./types";

/** The session state of the page: loading, signed out, signed in, or unavailable. */
export type SessionState =
  | { kind: "loading" }
  | { kind: "off" }
  | { kind: "signed_out" }
  | { kind: "signed_in"; me: Me }
  | { kind: "error"; message: string };

/** Reads the session once and offers sign-out. */
export const useSession = () => {
  const [state, setState] = useState<SessionState>(engineUrl() ? { kind: "loading" } : { kind: "off" });

  useEffect(() => {
    if (!engineUrl()) return;
    let live = true;
    fetchMe()
      .then((me) => live && setState(me ? { kind: "signed_in", me } : { kind: "signed_out" }))
      .catch((e: unknown) => live && setState({ kind: "error", message: String(e) }));
    return () => {
      live = false;
    };
  }, []);

  const signOut = useCallback(() => {
    logout()
      .catch(() => undefined)
      .finally(() => setState({ kind: "signed_out" }));
  }, []);

  return { state, signOut };
};
