import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../../App";

const ENGINE = "https://engine.example.org";
const MOD = { user_id: "111", name: "Mod", roles: ["MANAGE_GUILD"], expires_at: 0 };

type Call = { url: string; method: string; body: unknown };

const engine = (routes: Record<string, (body: unknown) => Response>) => {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      const method = init?.method ?? "GET";
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;
      calls.push({ url, method, body });
      const route = routes[`${method} ${url.replace(ENGINE, "")}`];
      return route ? route(body) : new Response("missing", { status: 404 });
    }),
  );
  return calls;
};

const open = (path: string) => {
  window.history.pushState(null, "", path);
  render(<App />);
};

describe("admin pages", () => {
  beforeEach(() => vi.stubEnv("VITE_ENGINE_URL", ENGINE));
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it("switches a tool off and shows the new state", async () => {
    let on = true;
    const calls = engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/tools": () => Response.json([{ name: "run_sandbox", description: "shell", on, private: false }]),
      "POST /admin/tools/run_sandbox": (body) => {
        on = (body as { on: boolean }).on;
        return Response.json({ id: 1 });
      },
    });
    open("/admin/tools");
    const toggle = await screen.findByRole("switch", { name: "Offer run_sandbox" });
    expect(toggle).toBeChecked();
    fireEvent.click(toggle);
    await waitFor(() => expect(screen.getByRole("switch", { name: "Offer run_sandbox" })).not.toBeChecked());
    expect(calls.find((c) => c.method === "POST")?.body).toEqual({ on: false });
  });

  it("saves edited instructions and returns to the configured ones", async () => {
    const calls = engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/instructions": () =>
        Response.json({ text: "Be brief.", default_text: "sys", changed: true, max_chars: 100 }),
      "PUT /admin/instructions": () => Response.json({ id: 2 }),
    });
    open("/admin/instructions");
    const box = await screen.findByLabelText("Instructions", { selector: "textarea" });
    fireEvent.change(box, { target: { value: "Be kind." } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    fireEvent.click(screen.getByRole("button", { name: /configured instructions/i }));
    await waitFor(() => expect(calls.filter((c) => c.method === "PUT").map((c) => c.body)).toEqual([{ text: "Be kind." }, { text: null }]));
  });

  it("rolls back to an earlier change from the history", async () => {
    const calls = engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/history": () =>
        Response.json([
          { id: 2, kind: "instructions", key: "system", value: "Be kind.", actor: "111", created_at: "2026-11-04" },
          { id: 1, kind: "tool", key: "run_sandbox", value: false, actor: "111", created_at: "2026-11-03" },
        ]),
      "POST /admin/rollback/1": () => Response.json({ id: 3 }),
    });
    open("/admin/history");
    expect(await screen.findByText("run_sandbox off")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Roll back to change 1" }));
    await waitFor(() => expect(calls.some((c) => c.method === "POST" && c.url.endsWith("/admin/rollback/1"))).toBe(true));
  });

  it("denies a held action", async () => {
    let held = [{ token: "t1", user_id: "222", tool: "gcal_create", created_at: "a", expires_at: "b" }];
    engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/approvals": () => Response.json(held),
      "POST /admin/approvals/t1/deny": () => {
        held = [];
        return Response.json({ denied: true });
      },
    });
    open("/admin/approvals");
    fireEvent.click(await screen.findByRole("button", { name: "Deny gcal_create for 222" }));
    expect(await screen.findByText("Nothing is waiting.")).toBeInTheDocument();
  });

  it("tells a member without the role why the page is empty and calls no admin route", async () => {
    const calls = engine({ "GET /web/me": () => Response.json({ ...MOD, roles: [] }) });
    open("/admin");
    expect(await screen.findByRole("alert")).toHaveTextContent(/manage server role/i);
    expect(calls.some((c) => c.url.includes("/admin/"))).toBe(false);
  });

  it("shows the refusal when the engine denies a moderator", async () => {
    engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/tools": () => new Response("this needs the server manager role", { status: 403 }),
    });
    open("/admin/tools");
    expect(await screen.findByRole("alert")).toHaveTextContent(/manage server role/i);
  });

  it("sums the newest traces on the overview", async () => {
    const row = (id: string, status: string | null, duration_ms: number, user_id = "222") => ({
      request_id: id,
      conversation_id: "c",
      at: "2026-11-04T00:00:00Z",
      user_id,
      input: `question ${id}`,
      status,
      steps: 1,
      tokens: 100,
      duration_ms,
    });
    engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/traces": () => Response.json([row("a", "answered", 1000), row("b", "error", 3000, "333"), row("c", "answered", 2000)]),
      "GET /admin/tools": () => Response.json([{ name: "x", description: "", on: true, private: false }, { name: "y", description: "", on: false, private: false }]),
      "GET /admin/approvals": () => Response.json([]),
      "GET /admin/history": () => Response.json([]),
    });
    open("/admin");
    const overview = within(await screen.findByRole("region", { name: "Overview" }));
    expect(await overview.findByText("67%")).toBeInTheDocument();
    expect(overview.getAllByText("2.0 s")).toHaveLength(2);
    expect(overview.getByText("1 / 2")).toBeInTheDocument();
    expect(overview.getByText("from 2 people")).toBeInTheDocument();
    expect(overview.getByText("question a")).toBeInTheDocument();
  });

  it("lists traces and opens one to its events", async () => {
    engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/traces": () =>
        Response.json([
          {
            request_id: "r1",
            conversation_id: "c1",
            at: "2026-11-04T00:00:00Z",
            user_id: "222",
            input: "when is the library open?",
            status: "answered",
            steps: 2,
            tokens: 120,
            duration_ms: 900,
          },
        ]),
      "GET /admin/traces/r1": () =>
        Response.json([
          { at: "t0", kind: "request_started", input: "when is the library open?" },
          { at: "t1", kind: "completed" },
        ]),
    });
    open("/admin/traces");
    fireEvent.click(await screen.findByRole("link", { name: "Open trace r1" }));
    expect(window.location.pathname).toBe("/admin/traces/r1");
    const events = within(await screen.findByRole("list", { name: "Trace events" }));
    expect(events.getByText("Request started")).toBeInTheDocument();
    expect(events.getByText("Completed")).toBeInTheDocument();
  });

  it("lists the live sources", async () => {
    engine({
      "GET /web/me": () => Response.json(MOD),
      "GET /admin/sources": () =>
        Response.json([{ key: "library_hours", indexed: true, params: [{ name: "date", required: false }] }]),
    });
    open("/admin/sources");
    expect(await screen.findByText("library_hours")).toBeInTheDocument();
    expect(screen.getByText("date")).toBeInTheDocument();
    expect(screen.getByText("indexed")).toBeInTheDocument();
  });
});
