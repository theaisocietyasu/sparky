# apps/web

Public landing site, plus the web chat (`/chat`), memory list (`/memories`) and the admin pages (`/admin`). Vite, React, TypeScript, Tailwind 4. Self-hosted fonts. Runtime dependencies: React, React Router, motion, lenis, lucide-react, and react-markdown with remark-gfm for answers.

```bash
just web          # dev server on :5173
just check-web    # eslint, tsc, vitest, build
```

The demo videos in `public/demo` are rendered from `src/demo` frame by frame: `CHROMIUM=/path/to/chrome npm run demo:record` (needs ffmpeg).

The build is static apart from one edge function: `api/engine.ts` forwards `/api/engine/*` to `ENGINE_URL` (`src/proxy/engine.ts`). `/chat`, `/memories` and `/admin` call the engine at the build-time `VITE_ENGINE_URL` (`/api/engine` on Vercel) with the Discord session cookie (`src/chat/`); they render the progress `text` the engine sends, with its leading mark shown as an icon (`src/chat/step.ts`). Without `VITE_ENGINE_URL` they say the chat is not set up.

The app pages share one layout (`src/app/`): `session.tsx` reads the session once for every page under it, `Frame.tsx` is the sidebar, and `theme.ts` keeps the light or dark choice. They load on first visit, so the landing page does not carry their code. Answers render as GitHub flavored markdown with no raw HTML and no remote images, and type out smoothly from the draft the engine streams (`src/chat/useSmoothText.ts`). Reduced motion turns the animations off. Deployment: see [deploy/README.md](../../deploy/README.md).
