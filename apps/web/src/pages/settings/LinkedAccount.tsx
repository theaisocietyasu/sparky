import { Check, type LucideIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { accountLink, accountStatus, EngineError, type Provider, unlinkAccount } from "../../chat/client";

type Status = "loading" | "linked" | "unlinked" | "off" | "error";

/** One calendar account in Connected accounts: its state, and Connect or Disconnect. */
const LinkedAccount = ({ provider, name, detail, icon: Icon }: { provider: Provider; name: string; detail: string; icon: LucideIcon }) => {
  const [status, setStatus] = useState<Status>("loading");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    accountStatus(provider)
      .then((linked) => setStatus(linked ? "linked" : "unlinked"))
      .catch((e: unknown) => setStatus(e instanceof EngineError && e.status === 503 ? "off" : "error"));
  }, [provider]);

  useEffect(load, [load]);

  const connect = async () => {
    setBusy(true);
    setError(null);
    try {
      window.location.assign(await accountLink(provider));
    } catch (e) {
      setError(e instanceof EngineError && e.status === 503 ? `${name} sign-in is not set up yet.` : "Could not start the sign-in.");
      setBusy(false);
    }
  };

  const disconnect = async () => {
    setBusy(true);
    setError(null);
    try {
      await unlinkAccount(provider);
      setStatus("unlinked");
    } catch {
      setError("Could not disconnect.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section aria-label={name} className="glass mt-4 rounded-2xl p-5">
      <div className="flex items-start gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-sunk">
          <Icon className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="font-medium">{name}</h2>
            {status === "linked" && (
              <span className="inline-flex items-center gap-1 rounded-md bg-good/15 px-1.5 py-0.5 text-xs font-medium text-good">
                <Check className="size-3" />
                Linked
              </span>
            )}
          </div>
          <p className="mt-1 text-sm text-dim">{status === "off" ? "Not on for this Sparky yet." : detail}</p>
        </div>
        {status === "loading" && <div aria-hidden className="h-9 w-24 animate-pulse rounded-lg bg-sunk" />}
        {status === "unlinked" && (
          <button
            type="button"
            onClick={connect}
            disabled={busy}
            className="h-9 rounded-lg bg-accent px-3 text-sm font-medium text-accent-fg hover:opacity-90 disabled:opacity-50"
          >
            Connect
          </button>
        )}
        {status === "linked" && (
          <button type="button" onClick={disconnect} disabled={busy} className="h-9 rounded-lg border border-edge px-3 text-sm hover:bg-sunk disabled:opacity-50">
            Disconnect
          </button>
        )}
      </div>
      {status === "error" && (
        <p role="alert" className="mt-3 text-sm text-bad">
          Could not check your {name} link.
        </p>
      )}
      {error && (
        <p role="alert" className="mt-3 text-sm text-bad">
          {error}
        </p>
      )}
    </section>
  );
};

export default LinkedAccount;
