import { LogOut } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useMe } from "../../app/useMe";
import { memories } from "../../chat/client";
import type { Memory } from "../../chat/types";

/** Settings, Profile: who is signed in, the facts Sparky knows about them, and sign out. */
const Profile = () => {
  const { me, signOut } = useMe();
  const [facts, setFacts] = useState<Memory[] | null>(null);

  useEffect(() => {
    let live = true;
    memories()
      .then((m) => live && setFacts(m.filter((x) => x.kind === "fact")))
      .catch(() => live && setFacts([]));
    return () => {
      live = false;
    };
  }, []);

  return (
    <section aria-labelledby="settings-title">
      <h2 id="settings-title" className="text-lg font-semibold tracking-tight">
        Profile
      </h2>
      <div className="glass mt-6 flex items-center gap-4 rounded-2xl p-5">
        <span aria-hidden className="grid size-12 shrink-0 place-items-center rounded-full bg-accent text-lg font-semibold text-accent-fg">
          {me.name.slice(0, 1).toUpperCase()}
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate font-medium">{me.name}</p>
          <p className="text-sm text-dim">Signed in with Discord</p>
        </div>
        <button type="button" onClick={signOut} className="inline-flex h-9 items-center gap-2 rounded-lg border border-edge px-3 text-sm hover:bg-sunk">
          <LogOut className="size-4" />
          Sign out
        </button>
      </div>

      <div className="glass mt-4 rounded-2xl p-5">
        <div className="flex items-baseline justify-between gap-4">
          <h3 className="font-medium">About you</h3>
          <Link to="/settings/memories" className="text-sm text-dim hover:text-fg">
            All memories
          </Link>
        </div>
        {facts === null && <div aria-hidden className="mt-3 h-12 animate-pulse rounded-lg bg-sunk" />}
        {facts?.length === 0 && <p className="mt-2 text-sm text-dim">Nothing yet. Tell Sparky your major, year and campus in a chat and it keeps them here.</p>}
        {facts && facts.length > 0 && (
          <ul aria-label="About you" className="mt-3 flex flex-wrap gap-2">
            {facts.map((f) => (
              <li key={f.id} className="rounded-lg bg-sunk px-2.5 py-1 text-sm">
                {f.text}
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
};

export default Profile;
