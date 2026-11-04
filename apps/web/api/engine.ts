import { forward } from "../src/proxy/engine";

/** The environment Vercel gives the function. */
declare const process: { env: Record<string, string | undefined> };

export const config = { runtime: "edge" };

/** Serves the engine under /api/engine on the site origin, so the session cookie is first-party. */
export default function handler(request: Request): Promise<Response> {
  return forward(request, process.env.ENGINE_URL);
}
