# apps/web

Public landing site, plus the web chat (`/chat`) and memory list (`/memories`). Vite, React, TypeScript, Tailwind 4. Self-hosted fonts. Runtime dependencies: React, React Router, motion, lenis and lucide-react.

```bash
just web          # dev server on :5173
just check-web    # eslint, tsc, vitest, build
```

The demo videos in `public/demo` are rendered from `src/demo` frame by frame: `CHROMIUM=/path/to/chrome npm run demo:record` (needs ffmpeg).

The build is static. `/chat` and `/memories` call the engine at the build-time `VITE_ENGINE_URL` with the Discord session cookie (`src/chat/`); they render the progress `text` the engine sends. Without `VITE_ENGINE_URL` they say the chat is not set up. Deployment: see [deploy/README.md](../../deploy/README.md).
