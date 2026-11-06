import { CalendarDays, CalendarRange, Check } from "lucide-react";
import { type FormEvent, useCallback, useEffect, useState } from "react";
import { canvasStatus, EngineError, removeCanvas, saveCanvas } from "../../chat/client";
import LinkedAccount from "./LinkedAccount";

type Status = "loading" | "linked" | "unlinked" | "off" | "error";

/** Settings, Connected accounts: Google Calendar and the Canvas calendar feed. */
const Accounts = () => {
  const [status, setStatus] = useState<Status>("loading");
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    canvasStatus()
      .then((linked) => setStatus(linked ? "linked" : "unlinked"))
      .catch((e: unknown) => setStatus(e instanceof EngineError && e.status === 503 ? "off" : "error"));
  }, []);

  useEffect(load, [load]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!link.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await saveCanvas(link.trim());
      setLink("");
      setStatus("linked");
    } catch (err) {
      setError(err instanceof EngineError && err.status === 422 ? err.message : "Could not save the link.");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await removeCanvas();
      setStatus("unlinked");
    } catch {
      setError("Could not remove the link.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section aria-labelledby="settings-title">
          <h2 id="settings-title" className="text-lg font-semibold tracking-tight">Connected accounts</h2>
          <p className="mt-1 text-sm text-dim">Accounts Sparky can read for you.</p>

          <section aria-label="Canvas" className="glass mt-6 rounded-2xl p-5">
            <div className="flex items-start gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-sunk">
                <CalendarDays className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h2 className="font-medium">Canvas</h2>
                  {status === "linked" && (
                    <span className="inline-flex items-center gap-1 rounded-md bg-good/15 px-1.5 py-0.5 text-xs font-medium text-good">
                      <Check className="size-3" />
                      Linked
                    </span>
                  )}
                </div>
                <p className="mt-1 text-sm text-dim">Due dates and course events from your Canvas calendar feed. No grades.</p>
              </div>
              {status === "linked" && (
                <button type="button" onClick={remove} disabled={busy} className="h-9 rounded-lg border border-edge px-3 text-sm hover:bg-sunk disabled:opacity-50">
                  Remove
                </button>
              )}
            </div>

            {status === "loading" && <div aria-hidden className="mt-4 h-9 animate-pulse rounded-lg bg-sunk" />}
            {status === "off" && <p className="mt-4 text-sm text-dim">Canvas is not on for this Sparky.</p>}
            {status === "error" && (
              <p role="alert" className="mt-4 text-sm text-bad">
                Could not check your Canvas link.
              </p>
            )}
            {(status === "unlinked" || status === "linked") && (
              <form onSubmit={save} className="mt-4">
                <label htmlFor="canvas-link" className="text-sm text-dim">
                  {status === "linked" ? "Replace the link" : "In Canvas, open Calendar, then Calendar Feed, and paste the link"}
                </label>
                <div className="mt-2 flex gap-2">
                  <input
                    id="canvas-link"
                    type="url"
                    value={link}
                    onChange={(e) => setLink(e.target.value)}
                    placeholder="https://canvas.asu.edu/feeds/calendars/user_....ics"
                    autoComplete="off"
                    className="h-9 min-w-0 flex-1 rounded-lg border border-edge bg-raised/60 px-3 text-sm outline-none focus:border-accent"
                  />
                  <button
                    type="submit"
                    disabled={busy || !link.trim()}
                    className="h-9 rounded-lg bg-accent px-3 text-sm font-medium text-accent-fg hover:opacity-90 disabled:opacity-50"
                  >
                    Save
                  </button>
                </div>
              </form>
            )}
            {error && (
              <p role="alert" className="mt-3 text-sm text-bad">
                {error}
              </p>
            )}
          </section>

          <LinkedAccount provider="google" name="Google Calendar" detail="Your events, read only, so Sparky can plan around them." icon={CalendarRange} />
    </section>
  );
};

export default Accounts;
