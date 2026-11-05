import { Suspense } from "react";
import { Outlet, useSearchParams } from "react-router-dom";
import { useSession } from "../chat/useSession";
import SignIn from "../components/chat/SignIn";
import { SessionContext } from "./useMe";

const Notice = ({ children, alert }: { children: string; alert?: boolean }) => (
  <main className="grid min-h-screen place-items-center bg-canvas px-4 text-fg">
    <p role={alert ? "alert" : undefined} className="text-sm text-dim">
      {children}
    </p>
  </main>
);

/** Layout route: reads the session once, shows sign-in without one, and gives its pages the student. */
const Signed = () => {
  const { state, signOut } = useSession();
  const [params] = useSearchParams();
  if (state.kind === "loading") return <Notice>Loading...</Notice>;
  if (state.kind === "off") return <Notice>The web chat is not set up on this site.</Notice>;
  if (state.kind === "error") return <Notice alert>Sparky is not reachable right now.</Notice>;
  if (state.kind === "signed_out") return <SignIn outcome={params.get("signin")} />;
  return (
    <SessionContext.Provider value={{ me: state.me, signOut }}>
      <Suspense fallback={<div className="min-h-screen bg-canvas" />}>
        <Outlet />
      </Suspense>
    </SessionContext.Provider>
  );
};

export default Signed;
