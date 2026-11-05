import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";

const ENGINE = "https://engine.example.org";
const ME = { user_id: "111", name: "Sparky Fan", roles: [], expires_at: 0 };

const sse = (frames: [string, unknown][]) =>
  new Response(frames.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify(data)}\n\n`).join(""), {
    status: 200,
    headers: { "Content-Type": "text/event-stream" },
  });

const answer = (extra: object = {}) => ({
  request_id: "r",
  conversation_id: "c1",
  text: "Hayden Library is open until midnight.",
  status: "answered",
  citations: [{ key: "lib", title: "ASU Library hours", url: "https://lib.asu.edu/hours" }],
  confirmation: null,
  memories: ["Studies at Tempe"],
  ...extra,
});

type Call = { url: string; init?: RequestInit };

const engine = (routes: Record<string, () => Response>) => {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      const route = routes[url.replace(ENGINE, "")];
      return route ? route() : new Response("missing", { status: 404 });
    }),
  );
  return calls;
};

const open = (path: string) => {
  window.history.pushState(null, "", path);
  render(<App />);
};

describe("web chat", () => {
  beforeEach(() => vi.stubEnv("VITE_ENGINE_URL", ENGINE));
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it("offers Discord sign-in without a session and says why the last one failed", async () => {
    engine({ "/web/me": () => new Response("", { status: 401 }) });
    open("/chat?signin=not_member");
    const link = await screen.findByRole("link", { name: /sign in with discord/i });
    expect(link).toHaveAttribute("href", `${ENGINE}/web/login`);
    expect(screen.getByRole("alert")).toHaveTextContent(/not in the server/i);
  });

  it("says the chat is off when the build names no engine", () => {
    vi.stubEnv("VITE_ENGINE_URL", "");
    open("/chat");
    expect(screen.getByText(/not set up/i)).toBeInTheDocument();
  });

  it("streams a turn with cookies and shows the answer, its sources and memories", async () => {
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () =>
        sse([
          ["progress", { text: "searching ASU", slot: "s" }],
          ["answer", answer()],
          ["done", {}],
        ]),
    });
    open("/chat");
    fireEvent.change(await screen.findByLabelText("Message"), { target: { value: "library hours?" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    const convo = within(screen.getByRole("list", { name: "Conversation" }));
    expect(await convo.findByText(/open until midnight/i)).toBeInTheDocument();
    expect(await convo.findByRole("link", { name: /asu library hours/i })).toHaveAttribute("href", "https://lib.asu.edu/hours");
    expect(convo.getByText(/memories used: 1/i)).toBeInTheDocument();

    const stream = calls.find((c) => c.url === `${ENGINE}/chat/stream`);
    expect(stream?.init?.credentials).toBe("include");
    expect(JSON.parse(String(stream?.init?.body))).toMatchObject({ message: "library hours?", conversation_id: null, continue_channel: false });
  });

  it("asks before a held action and sends the answer to /confirm", async () => {
    const held = { token: "t1", tool: "gcal_create", summary: "Add Study group to your calendar" };
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () => sse([["answer", answer({ text: "Shall I?", confirmation: held, citations: [] })]]),
      "/confirm": () => Response.json(answer({ text: "Added.", citations: [], memories: [] })),
    });
    open("/chat");
    fireEvent.change(await screen.findByLabelText("Message"), { target: { value: "add study group" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    fireEvent.click(await screen.findByRole("button", { name: "Approve" }));

    expect(await screen.findByText("Added.")).toBeInTheDocument();
    const sent = calls.find((c) => c.url === `${ENGINE}/confirm`);
    expect(JSON.parse(String(sent?.init?.body))).toMatchObject({ token: "t1", approve: true, conversation_id: "c1" });
    expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
  });

  it("renders markdown in the answer and keeps unsafe links as text", async () => {
    engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () =>
        sse([["answer", answer({ text: "**Hours**\n\n- Mon: 7am\n- Tue: 7am\n\n```sh\nls\n```\n\n[bad](javascript:alert(1))", citations: [], memories: [] })]]),
    });
    open("/chat");
    fireEvent.click(await screen.findByRole("button", { name: "When is Hayden Library open today?" }));
    const convo = within(await screen.findByRole("list", { name: "Conversation" }));
    expect(await convo.findByRole("button", { name: "Copy answer" })).toBeInTheDocument();
    expect(convo.getByRole("button", { name: "Copy code" })).toBeInTheDocument();
    expect(convo.getByText("Hours").tagName).toBe("STRONG");
    expect(convo.getByText("Mon: 7am").tagName).toBe("LI");
    expect(convo.getByText("bad").closest("a")).toBeNull();
  });

  it("shows an error frame as a message instead of an answer", async () => {
    engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () => sse([["error", { error: "busy", status: 503 }]]),
    });
    open("/chat");
    fireEvent.change(await screen.findByLabelText("Message"), { target: { value: "hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/sparky is busy/i);
  });

  it("lists memories and forgets one by its number", async () => {
    let listed = [
      { number: 1, id: "a", kind: "fact", text: "Studies at Tempe", created_at: "2026-11-01" },
      { number: 2, id: "b", kind: "task", text: "Essay", created_at: "2026-11-02", due: "2026-11-09" },
    ];
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/memories/list": () => Response.json({ memories: listed }),
      "/memories/forget": () => {
        listed = listed.slice(1);
        return Response.json({ removed: 1 });
      },
    });
    open("/memories");
    expect(await screen.findByText("Studies at Tempe")).toBeInTheDocument();
    expect(screen.getByText(/due 2026-11-09/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Forget memory 1" }));
    await waitFor(() => expect(screen.queryByText("Studies at Tempe")).toBeNull());
    const sent = calls.find((c) => c.url === `${ENGINE}/memories/forget`);
    expect(JSON.parse(String(sent?.init?.body))).toMatchObject({ number: 1 });
  });

  it("draws memories as a tree by kind and switches to the list", async () => {
    engine({
      "/web/me": () => Response.json(ME),
      "/memories/list": () =>
        Response.json({
          memories: [
            { number: 1, id: "a", kind: "fact", text: "Studies at Tempe", created_at: "2026-11-01" },
            { number: 2, id: "b", kind: "fact", text: "Majors in CS", created_at: "2026-11-01" },
            { number: 3, id: "c", kind: "task", text: "Essay", created_at: "2026-11-02", due: "2026-11-09" },
          ],
        }),
    });
    open("/memories");
    const map = await screen.findByRole("list", { name: "Memory map" });
    const facts = within(map).getByRole("list", { name: "Facts" });
    expect(within(facts).getAllByRole("listitem")).toHaveLength(2);
    expect(within(map).getByRole("list", { name: "Tasks" })).toHaveTextContent(/due 2026-11-09/);
    fireEvent.click(screen.getByRole("button", { name: "List" }));
    expect(screen.getByRole("list", { name: "Memories" })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Memory map" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Map" }));
  });

  it("lists chats, reopens one with its turns and deletes it", async () => {
    let listed = [{ id: "c9", title: "When is my exam", updated_at: "2026-11-05" }];
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/web/conversations": () => Response.json(listed),
      "/web/conversations/c9": () => {
        listed = [];
        return Response.json([
          { role: "user", text: "When is my exam" },
          { role: "assistant", text: "Friday at 9." },
        ]);
      },
    });
    open("/chat");
    const sidebar = within(await screen.findByRole("list", { name: "Chats" }));
    fireEvent.click(sidebar.getByRole("link", { name: "When is my exam" }));
    expect(await screen.findByText("Friday at 9.")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/chat/c9");

    fireEvent.click(sidebar.getByRole("button", { name: "Delete chat When is my exam" }));
    await waitFor(() => expect(window.location.pathname).toBe("/chat"));
    const removed = calls.find((c) => c.url === `${ENGINE}/web/conversations/c9` && c.init?.method === "DELETE");
    expect(removed).toBeDefined();
  });

  it("links Canvas from the Connections page", async () => {
    let linked = false;
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/canvas/feed": () => {
        if (calls[calls.length - 1]?.init?.method === "POST") linked = true;
        return Response.json(linked ? { linked: true, saved: true } : { linked: false });
      },
      "/canvas/feed/remove": () => Response.json({ removed: true }),
    });
    open("/connections");
    const link = "https://canvas.asu.edu/feeds/calendars/user_A.ics";
    fireEvent.change(await screen.findByLabelText(/calendar feed/i), { target: { value: link } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Linked")).toBeInTheDocument();
    const saved = calls.find((c) => c.url === `${ENGINE}/canvas/feed` && c.init?.method === "POST");
    expect(JSON.parse(String(saved?.init?.body))).toEqual({ link });

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(screen.queryByText("Linked")).toBeNull());
  });
});
