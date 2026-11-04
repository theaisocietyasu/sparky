import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    const toggle = await screen.findByRole("checkbox", { name: "Offer run_sandbox" });
    expect(toggle).toBeChecked();
    fireEvent.click(toggle);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Offer run_sandbox" })).not.toBeChecked());
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

  it("tells a member without the role why the page is empty", async () => {
    engine({
      "GET /web/me": () => Response.json({ ...MOD, roles: [] }),
      "GET /admin/tools": () => new Response("this needs the server manager role", { status: 403 }),
    });
    open("/admin");
    expect(await screen.findByRole("alert")).toHaveTextContent(/manage server role/i);
  });
});
