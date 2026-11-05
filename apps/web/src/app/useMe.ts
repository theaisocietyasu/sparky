import { createContext, useContext } from "react";
import type { Me } from "../chat/types";

/** The signed-in student and sign-out. */
export type Session = { me: Me; signOut: () => void };

/** Holds the session for pages under the Signed route. */
export const SessionContext = createContext<Session | null>(null);

/** The signed-in student and sign-out, for pages under the Signed route. */
export const useMe = () => {
  const session = useContext(SessionContext);
  if (!session) throw new Error("useMe is used outside the Signed route");
  return session;
};

/** Whether the student may open the admin pages. */
export const isModerator = (me: Me) => me.roles.length > 0;
