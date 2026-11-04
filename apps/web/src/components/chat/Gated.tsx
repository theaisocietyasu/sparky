import type { ReactNode } from "react";
import { useSearchParams } from "react-router-dom";
import type { Me } from "../../chat/types";
import { useSession } from "../../chat/useSession";
import Shell from "./Shell";
import SignIn from "./SignIn";

/** Renders children for a signed-in student and the sign-in card otherwise. */
const Gated = ({ children }: { children: (me: Me) => ReactNode }) => {
  const { state, signOut } = useSession();
  const [params] = useSearchParams();
  const me = state.kind === "signed_in" ? state.me : null;
  return (
    <Shell me={me} onSignOut={signOut}>
      {state.kind === "loading" && <p className="text-sm text-ink-soft">Loading...</p>}
      {state.kind === "off" && <p className="text-sm text-ink-soft">The web chat is not set up on this site.</p>}
      {state.kind === "error" && (
        <p role="alert" className="text-sm text-shu">
          Sparky is not reachable right now.
        </p>
      )}
      {state.kind === "signed_out" && <SignIn outcome={params.get("signin")} />}
      {me && children(me)}
    </Shell>
  );
};

export default Gated;
