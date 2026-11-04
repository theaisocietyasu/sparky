import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import type { Me } from "../../chat/types";

/** Page frame for the signed-in pages: the header with the student and the page links. */
const Shell = ({ me, onSignOut, children }: { me: Me | null; onSignOut?: () => void; children: ReactNode }) => (
  <div className="flex min-h-screen flex-col bg-paper text-ink">
    <header className="border-b border-ink/10">
      <nav aria-label="Chat" className="mx-auto flex h-14 max-w-3xl items-center gap-5 px-4 text-sm">
        <Link to="/" className="flex items-center gap-2 font-serif text-base font-bold">
          <img src="/brand/sparkyai-logo.png" alt="" width={717} height={779} className="h-7 w-auto" />
          Sparky
        </Link>
        {me && (
          <>
            <Link to="/chat" className="text-ink-soft hover:text-ink">
              Chat
            </Link>
            <Link to="/memories" className="text-ink-soft hover:text-ink">
              Memories
            </Link>
            {me.roles.length > 0 && (
              <Link to="/admin" className="text-ink-soft hover:text-ink">
                Admin
              </Link>
            )}
            <span className="ml-auto truncate text-ink-soft">{me.name}</span>
            <button type="button" onClick={onSignOut} className="font-medium text-shu hover:text-shu-deep">
              Sign out
            </button>
          </>
        )}
      </nav>
    </header>
    <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 py-6">{children}</main>
  </div>
);

export default Shell;
