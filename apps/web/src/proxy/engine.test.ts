import { describe, expect, it, vi } from "vitest";
import { forward, target } from "./engine";

const ENGINE = "https://pod-8080.proxy.runpod.net/";

describe("target", () => {
  it("maps the rewrite query and the plain path onto the engine", () => {
    expect(target(new URL("https://s.org/api/engine?__path=web/callback&code=1&state=x"), ENGINE)).toBe(
      "https://pod-8080.proxy.runpod.net/web/callback?code=1&state=x",
    );
    expect(target(new URL("https://s.org/api/engine/web/me"), ENGINE)).toBe(
      "https://pod-8080.proxy.runpod.net/web/me",
    );
  });

  it("refuses paths outside the prefix or with dot segments", () => {
    expect(target(new URL("https://s.org/other"), ENGINE)).toBeNull();
    expect(target(new URL("https://s.org/api/engine?__path=../admin"), ENGINE)).toBeNull();
  });
});

describe("forward", () => {
  it("passes cookies, the browser origin headers and redirects through", async () => {
    const send = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      const sent = new Headers(init?.headers);
      expect(sent.get("cookie")).toBe("sparky_session=abc");
      expect(sent.get("sec-fetch-site")).toBe("same-origin");
      expect(sent.has("host")).toBe(false);
      expect(init?.redirect).toBe("manual");
      const headers = new Headers({ location: "https://discord.com/oauth2/authorize", "content-encoding": "gzip" });
      headers.append("set-cookie", "a=1; Path=/api/engine/web");
      headers.append("set-cookie", "b=2; Path=/");
      return new Response(null, { status: 303, headers });
    });
    const request = new Request("https://s.org/api/engine?__path=web/login", {
      headers: { cookie: "sparky_session=abc", "sec-fetch-site": "same-origin", host: "s.org" },
    });
    const res = await forward(request, ENGINE, send as unknown as typeof fetch);
    expect(send).toHaveBeenCalledWith("https://pod-8080.proxy.runpod.net/web/login", expect.anything());
    expect(res.status).toBe(303);
    expect(res.headers.get("location")).toBe("https://discord.com/oauth2/authorize");
    expect(res.headers.getSetCookie()).toEqual(["a=1; Path=/api/engine/web", "b=2; Path=/"]);
    expect(res.headers.has("content-encoding")).toBe(false);
  });

  it("answers 503 without an engine and 502 when it is down", async () => {
    const request = new Request("https://s.org/api/engine/web/me");
    expect((await forward(request, undefined)).status).toBe(503);
    const down = vi.fn(async () => {
      throw new TypeError("fetch failed");
    });
    expect((await forward(request, ENGINE, down as unknown as typeof fetch)).status).toBe(502);
  });
});
