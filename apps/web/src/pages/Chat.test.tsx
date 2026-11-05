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

  it("streams a turn with cookies and shows the answer and its sources, not the memories it used", async () => {
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
    expect(convo.queryByText(/memories used/i)).toBeNull();

    const stream = calls.find((c) => c.url === `${ENGINE}/chat/stream`);
    expect(stream?.init?.credentials).toBe("include");
    expect(JSON.parse(String(stream?.init?.body))).toMatchObject({ message: "library hours?", conversation_id: null, continue_channel: false });
  });

  it("shows one status line that changes in place while a turn runs", async () => {
    let finish: (r: Response) => void = () => {};
    engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () => {
        const body = new ReadableStream({
          start(c) {
            const enc = new TextEncoder();
            const frame = (name: string, data: unknown) => c.enqueue(enc.encode(`event: ${name}\ndata: ${JSON.stringify(data)}\n\n`));
            frame("progress", { text: "\u{1f527} Searching ASU pages", slot: "a" });
            frame("progress", { text: "\u{2705} Checked ASU pages", slot: "a" });
            finish = () => {
              frame("answer", answer());
              frame("done", {});
              c.close();
            };
          },
        });
        return new Response(body, { status: 200, headers: { "Content-Type": "text/event-stream" } });
      },
    });
    open("/chat");
    fireEvent.change(await screen.findByLabelText("Message"), { target: { value: "library hours?" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    const progress = await screen.findByLabelText("Progress");
    await waitFor(() => expect(progress).toHaveTextContent("Checked ASU pages"));
    expect(progress).not.toHaveTextContent("Searching ASU pages");
    finish(new Response());
    expect(await screen.findByText(/open until midnight/i)).toBeInTheDocument();
  });

  it("opens settings, the theme and sign out from the profile menu", async () => {
    engine({ "/web/me": () => Response.json(ME), "/memories/list": () => Response.json({ memories: [] }) });
    open("/chat");
    fireEvent.click(await screen.findByRole("button", { name: "Account menu" }));
    const menu = within(screen.getByRole("menu", { name: "Account" }));
    expect(menu.getByRole("menuitem", { name: /sign out/i })).toBeInTheDocument();
    expect(menu.queryByRole("menuitem", { name: "Admin" })).toBeNull();
    fireEvent.click(menu.getByRole("menuitem", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Profile" })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings");
    expect(screen.queryByRole("link", { name: "Connections" })).toBeNull();
  });

  it("shows a dot for new reminders and marks them read when the bell opens", async () => {
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/web/notifications": () =>
        Response.json({
          notifications: [{ id: "n1", text: "Submit the lab", created_at: new Date().toISOString(), read: false, kind: "reminder", helpful: null }],
          unread: 1,
        }),
      "/web/notifications/read": () => new Response(null, { status: 204 }),
    });
    open("/chat");
    const bell = await screen.findByRole("button", { name: "Notifications, 1 new" });
    fireEvent.click(bell);
    expect(within(screen.getByRole("dialog", { name: "Notifications" })).getByText("Submit the lab")).toBeInTheDocument();
    await waitFor(() => expect(calls.some((c) => c.url === `${ENGINE}/web/notifications/read` && c.init?.method === "POST")).toBe(true));
    expect(screen.getByRole("button", { name: "Notifications" })).toBeInTheDocument();
  });

  it("lists reminders in settings and cancels one", async () => {
    let listed = [{ id: "r1", text: "Gym", due_at: "2026-11-06T16:00:00Z", repeat: "daily" }];
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/web/reminders": () => Response.json({ reminders: listed }),
      "/web/reminders/r1": () => {
        listed = [];
        return new Response(null, { status: 204 });
      },
    });
    open("/settings/notifications");
    expect(await screen.findByText("Gym")).toBeInTheDocument();
    expect(screen.getByText(/repeats daily/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel reminder Gym" }));
    expect(await screen.findByText("No reminders set")).toBeInTheDocument();
    expect(calls.some((c) => c.url === `${ENGINE}/web/reminders/r1` && c.init?.method === "DELETE")).toBe(true);
  });

  it("switches a check-in kind off from the bell and in settings", async () => {
    const prefs = { quiet_start: 22, quiet_end: 8, off: [] as string[] };
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/web/notifications": () =>
        Response.json({
          notifications: [{ id: "n2", text: "Hackathon Friday", created_at: new Date().toISOString(), read: true, kind: "event", helpful: null }],
          unread: 0,
        }),
      "/web/notifications/n2/feedback": () => Response.json({ turned_off: "event" }),
      "/web/nudges": () => Response.json(prefs),
      "/web/reminders": () => Response.json({ reminders: [] }),
    });
    open("/chat");
    fireEvent.click(await screen.findByRole("button", { name: "Notifications" }));
    fireEvent.click(await within(screen.getByRole("dialog", { name: "Notifications" })).findByRole("button", { name: "Not useful" }));
    expect(await screen.findByText(/No more events for you/)).toBeInTheDocument();
    expect(calls.some((c) => c.url === `${ENGINE}/web/notifications/n2/feedback` && c.init?.method === "POST")).toBe(true);
  });

  it("saves check-in switches in settings", async () => {
    let saved: unknown = null;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        const path = url.replace(ENGINE, "");
        if (path === "/web/me") return Response.json(ME);
        if (path === "/web/reminders") return Response.json({ reminders: [] });
        if (path === "/web/nudges" && init?.method === "PUT") {
          saved = JSON.parse(String(init.body));
          return Response.json(saved);
        }
        if (path === "/web/nudges") return Response.json({ quiet_start: 22, quiet_end: 8, off: [] });
        return new Response("missing", { status: 404 });
      }),
    );
    open("/settings/notifications");
    fireEvent.click(await screen.findByRole("switch", { name: "Morning brief" }));
    await waitFor(() => expect(saved).toEqual({ quiet_start: 22, quiet_end: 8, off: ["brief"] }));
    expect(screen.getByRole("switch", { name: "Morning brief" })).toHaveAttribute("aria-checked", "false");
  });

  it("shows choices under the latest answer and sends the one tapped", async () => {
    let asked = 0;
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () => {
        asked += 1;
        const text = asked === 1 ? "Which campus are you on?\n\n```choices\nTempe\nWest\n```" : "Got it, Tempe.";
        return sse([
          ["answer", answer({ text, citations: [] })],
          ["done", {}],
        ]);
      },
    });
    open("/chat");
    fireEvent.change(await screen.findByLabelText("Message"), { target: { value: "hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    const choices = within(await screen.findByRole("list", { name: "Choices" }));
    expect(screen.queryByText(/```/)).toBeNull();
    fireEvent.click(choices.getByRole("button", { name: "Tempe" }));
    expect(await screen.findByText("Got it, Tempe.")).toBeInTheDocument();
    const second = calls.filter((c) => c.url === `${ENGINE}/chat/stream`)[1];
    expect(JSON.parse(String(second?.init?.body))).toMatchObject({ message: "Tempe" });
    expect(screen.queryByRole("list", { name: "Choices" })).toBeNull();
  });

  it("sends attached files inline and shows them on the turn", async () => {
    const calls = engine({
      "/web/me": () => Response.json(ME),
      "/chat/stream": () => sse([["answer", answer({ text: "The midterm is October 14." })], ["done", {}]]),
    });
    open("/chat");
    const picker = await screen.findByLabelText("Attach files");
    const notes = new File(["Midterm: October 14"], "notes.txt", { type: "text/plain" });
    fireEvent.change(picker, { target: { files: [notes] } });
    expect(within(screen.getByRole("list", { name: "Attached files" })).getByText("notes.txt")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    expect(await screen.findByText(/midterm is october 14/i)).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Files sent" })).getByText("notes.txt")).toBeInTheDocument();
    const stream = calls.find((c) => c.url === `${ENGINE}/chat/stream`);
    const body = JSON.parse(String(stream?.init?.body));
    expect(body.message).toBe("");
    expect(body.files).toEqual([{ name: "notes.txt", media_type: "text/plain", size: 19, data: btoa("Midterm: October 14") }]);
  });

  it("refuses a file over 2 MB before sending", async () => {
    engine({ "/web/me": () => Response.json(ME) });
    open("/chat");
    const picker = await screen.findByLabelText("Attach files");
    const big = new File([new Uint8Array(2_000_001)], "big.pdf", { type: "application/pdf" });
    fireEvent.change(picker, { target: { files: [big] } });
    expect(screen.getByRole("alert")).toHaveTextContent(/up to 2 MB/i);
    expect(screen.queryByRole("list", { name: "Attached files" })).toBeNull();
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

  it("links Canvas from Connected accounts in settings", async () => {
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
