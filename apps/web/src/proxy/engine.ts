/** Path prefix the site serves the engine under. */
export const PREFIX = "/api/engine";

/** Query key the Vercel rewrite puts the engine path in. */
export const PATH_KEY = "__path";

/** Response headers that describe the upstream transfer, not the body the proxy sends. */
const HOP = ["connection", "content-encoding", "content-length", "keep-alive", "transfer-encoding"];

/** Request headers the proxy sets itself or must not pass on. */
const DROP = ["connection", "host", "content-length"];

/** The engine URL for a request to the site, or null when the path leaves the prefix. */
export const target = (url: URL, engine: string): string | null => {
  const query = new URLSearchParams(url.search);
  const rewritten = query.get(PATH_KEY);
  query.delete(PATH_KEY);
  let path: string;
  if (rewritten !== null) {
    path = `/${rewritten.replace(/^\/+/, "")}`;
  } else if (url.pathname === PREFIX || url.pathname.startsWith(`${PREFIX}/`)) {
    path = url.pathname.slice(PREFIX.length) || "/";
  } else {
    return null;
  }
  if (path.split("/").some((part) => part === ".." || part === ".")) return null;
  const rest = query.toString();
  return `${engine.replace(/\/$/, "")}${path}${rest ? `?${rest}` : ""}`;
};

/** Sends a site request to the engine and streams the answer back, redirects and cookies included. */
export const forward = async (
  request: Request,
  engine: string | undefined,
  send: typeof fetch = fetch,
): Promise<Response> => {
  if (!engine) return new Response("the engine is not set up", { status: 503 });
  const url = target(new URL(request.url), engine);
  if (!url) return new Response("not found", { status: 404 });
  const headers = new Headers(request.headers);
  for (const name of DROP) headers.delete(name);
  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  let upstream: Response;
  try {
    upstream = await send(url, {
      method: request.method,
      headers,
      body: hasBody ? request.body : undefined,
      redirect: "manual",
      ...(hasBody ? { duplex: "half" } : {}),
    } as RequestInit);
  } catch {
    return new Response("the engine is not reachable", { status: 502 });
  }
  const out = new Headers(upstream.headers);
  for (const name of HOP) out.delete(name);
  return new Response(upstream.body, { status: upstream.status, statusText: upstream.statusText, headers: out });
};
