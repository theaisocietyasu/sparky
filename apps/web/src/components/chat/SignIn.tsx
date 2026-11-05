import { loginUrl } from "../../chat/client";

const NOTES: Record<string, string> = {
  not_member: "That Discord account is not in the server Sparky serves.",
  expired: "The sign-in took too long. Try again.",
  cancelled: "The sign-in was cancelled.",
  failed: "Discord did not answer. Try again.",
};

const DiscordMark = () => (
  <svg aria-hidden viewBox="0 0 24 24" className="size-5 fill-current">
    <path d="M20.3 4.4A19.6 19.6 0 0 0 15.4 3l-.6 1.3a18.2 18.2 0 0 0-5.6 0L8.6 3a19.5 19.5 0 0 0-4.9 1.5C.6 9.1-.3 13.6.1 18.1a19.7 19.7 0 0 0 6 3l1.3-2a12.8 12.8 0 0 1-2-1l.5-.4a14 14 0 0 0 12.2 0l.5.4c-.6.4-1.3.7-2 1l1.3 2a19.6 19.6 0 0 0 6-3c.5-5.2-.9-9.7-3.6-13.7ZM8 15.4c-1.2 0-2.2-1.1-2.2-2.4s1-2.4 2.2-2.4 2.2 1.1 2.2 2.4-1 2.4-2.2 2.4Zm8 0c-1.2 0-2.2-1.1-2.2-2.4s1-2.4 2.2-2.4 2.2 1.1 2.2 2.4-1 2.4-2.2 2.4Z" />
  </svg>
);

/** The sign-in screen, with the outcome of the last attempt when there was one. */
const SignIn = ({ outcome }: { outcome: string | null }) => (
  <main className="grid min-h-screen place-items-center bg-canvas px-4 text-fg">
    <div className="flex w-full max-w-sm flex-col items-center text-center">
      <img src="/brand/sparkyai-logo.png" alt="" width={717} height={779} className="h-14 w-auto" />
      <h1 className="mt-6 text-2xl font-semibold tracking-tight">Chat with Sparky</h1>
      <p className="mt-2 text-sm text-dim">Sign in with the Discord account you use in the server.</p>
      {outcome && NOTES[outcome] && (
        <p role="alert" className="mt-5 w-full rounded-lg border border-bad/30 bg-bad/10 px-3 py-2 text-sm text-bad">
          {NOTES[outcome]}
        </p>
      )}
      <a
        href={loginUrl()}
        className="mt-6 inline-flex h-11 w-full items-center justify-center gap-2 rounded-xl bg-[#5865f2] text-sm font-medium text-white transition-colors hover:bg-[#4752c4] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#5865f2]"
      >
        <DiscordMark />
        Sign in with Discord
      </a>
      <p className="mt-6 text-xs text-dim">Sparky reads only your Discord name and the servers you are in.</p>
    </div>
  </main>
);

export default SignIn;
