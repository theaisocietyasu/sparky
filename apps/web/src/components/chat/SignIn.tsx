import { loginUrl } from "../../chat/client";

const NOTES: Record<string, string> = {
  not_member: "That Discord account is not in the server Sparky serves.",
  expired: "The sign-in took too long. Try again.",
  cancelled: "The sign-in was cancelled.",
  failed: "Discord did not answer. Try again.",
};

/** The sign-in card, with the outcome of the last attempt when there was one. */
const SignIn = ({ outcome }: { outcome: string | null }) => (
  <div className="mx-auto mt-16 flex max-w-sm flex-col items-center rounded-md border-2 border-ink px-8 py-10 text-center">
    <h1 className="font-serif text-2xl font-bold">Chat with Sparky</h1>
    <p className="mt-3 text-sm text-ink-soft">Sign in with the Discord account you use in the server.</p>
    {outcome && NOTES[outcome] && (
      <p role="alert" className="mt-4 text-sm text-shu">
        {NOTES[outcome]}
      </p>
    )}
    <a
      href={loginUrl()}
      className="mt-6 inline-flex h-11 items-center rounded-full bg-shu px-6 text-sm font-semibold text-paper hover:bg-shu-deep"
    >
      Sign in with Discord
    </a>
  </div>
);

export default SignIn;
