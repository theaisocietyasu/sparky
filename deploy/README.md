# Deploy

## Local

```bash
just bootstrap     # .env, git hooks, deps
just engine        # then in separate shells: just discord, just web
just up            # the compose stack, built locally
```

The engine needs a running Platform: set `SPARKY_PLATFORM__URL`, `SPARKY_PLATFORM__TOKEN`, and `SPARKY_PLATFORM__MCP_URL` in `.env`. `just up` starts engine, discord and `sandboxd`. `engine` and `discord` share one image (`rust.Dockerfile`) with different entrypoints. `just model` starts the chat and embed models (profile `model`).

## Production

```bash
git clone https://github.com/theaisocietyasu/sparky.git && cd sparky
cp .env.example .env               # secrets and per-machine URLs; SPARKY_APP__ENV=production
SPARKY_IMAGE_TAG=main just prod-up # pulls ghcr.io images
just prod-logs engine
```

`deploy/compose.prod.yml` overrides `compose.yml` with prebuilt images and no host ports except engine `:8080`. Phoenix, Prometheus, and Grafana are reachable only over a tunnel. Put a reverse proxy with TLS in front of the engine. Settings live in the committed `sparky.toml`.

## RunPod

`deploy/docker/runpod.Dockerfile` builds `ghcr.io/theaisocietyasu/sparkyai-runpod`: all of Sparky in one GPU pod. A RunPod pod cannot start containers, so supervisord (`deploy/runpod/supervisord.conf`) runs chat and embed `llama-server`, the engine, and the bot as processes. Knowledge search, live queries and linked accounts live on Platform; conversations, memories and confirmations are in the engine SQLite file on the volume. It differs from compose in two ways:

- `run_sandbox` runs as WebAssembly inside the engine (`SPARKY_SANDBOX__RUNTIME=wasm`, no network). The image carries the packages in `deploy/wasm/packages.txt`; compiled modules and student folders live in `/workspace/sandbox`. See [The sandbox](#the-sandbox).
- Phoenix runs only with `SPARKY_RUN_PHOENIX=true`. Expose `6006/http` too. It signs in with `admin@localhost` and `admin` and asks for a new password at once, so sign in right after the first start. Its database and keys are in `/workspace/phoenix`, traces are kept `SPARKY_PHOENIX_RETENTION_DAYS` days (30), and the engine and the bot export to it with its admin key. The admin trace list links each request to its page there.

Create the pod with the image, a volume at `/workspace` (the model cache), a host CUDA version of at least 12.8, and these environment variables: `SPARKY_DISCORD__TOKEN`, `SPARKY_DISCORD__GUILD_ID`, `SPARKY_ENGINE__SERVICE_TOKEN`, `SPARKY_PLATFORM__URL`, `SPARKY_PLATFORM__TOKEN`, `SPARKY_PLATFORM__MCP_URL`, `SPARKY_MEMORY__KEY`, `SPARKY_APP__ENV=production`, and `HF_TOKEN` (a read token; community hosts share an IP that the Hugging Face Hub rate-limits without one). Every service listens on `127.0.0.1`; the pod exposes no port. The first boot downloads the GGUFs into `/workspace/models`. The scopes the Platform token needs are listed in the `[platform]` section of `sparky.toml`.

`SPARKY_MODELS_API_KEY` makes both model servers listen on every interface and require that key. Expose `8000/http` (chat) and `8001/http` (embed) on the pod to let other apps use them: Platform's `EMBEDDINGS_URL` for indexing and search, and Hermes or another agent for chat. The engine uses the same key.

### Models only

With `SPARKY_POD_ROLE=models` the pod runs only the two model servers, and `SPARKY_MODELS_API_KEY` is required. Use it on a GPU pod when the engine and the bot run elsewhere, for example on the Platform pod, which runs them against any OpenAI-compatible API. Point that engine at `https://<pod>-8000.proxy.runpod.net/v1` with the same key, and Platform's embeddings at `https://<pod>-8001.proxy.runpod.net/v1`.

### Apps only

With `SPARKY_POD_ROLE=apps` the pod runs only the engine and the bot, so it fits a CPU pod. Set `SPARKY_MODEL__BASE_URL`, `SPARKY_MODEL__API_KEY` and `SPARKY_MODEL__NAME` to an OpenAI-compatible API whose model calls tools, for example ASU's `https://openai.rc.asu.edu/v1` (keys and models on the Voyager portal, LLM Access tab). The pod refuses to start when `SPARKY_MODEL__BASE_URL` is unset or points at `127.0.0.1`.

### GPU deployment

This pod was terminated on 2026-10-08. The engine and the bot now run on the Platform pod with a hosted model API ("Agents on the platform pod" in the Platform `docs/operations.md`). This section records the GPU setup for a later GPU pod, with `SPARKY_POD_ROLE=models` or without it.

| | |
|---|---|
| Pod | `sparky`, id `gdrbqqc1vsw6cv`, community cloud, always on |
| GPU | 1x NVIDIA GeForce RTX 3070, 7840 MiB VRAM, driver 580.65.06, host CUDA 13.0 |
| Host | 22 vCPU and 24 GB RAM allocated by RunPod |
| Disk | 30 GB container disk (wiped on restart), 40 GB persistent volume at `/workspace` |
| Image | `ghcr.io/ashworks1706/sparkyai-runpod:590f3a1ac7d18a26738c0ecbe3988e9fa113aeb5` |
| Ports | none exposed; every service binds `127.0.0.1` |

GPU memory, from `nvidia-smi` on the pod:

| Process | Model | Context | VRAM |
|---|---|---|---|
| chat | `Qwen/Qwen3-4B-GGUF:Q4_K_M` | 16384, 2 slots of 8192 | 3892 MiB |
| embed | `Qwen/Qwen3-Embedding-0.6B-GGUF:Q8_0` | 4096, 2 slots of 2048 | 1846 MiB |
| total | | | 5753 of 7840 MiB |

The RTX 3070 was the cheapest community GPU in stock with host CUDA 12.8 or newer that holds both models. The next options were the RTX 3080 and RTX A4000 at $0.17/hr and the RTX 3090 at $0.22/hr. A larger chat model or context needs one of those.

Cost:

| | |
|---|---|
| Compute | $0.13/hr: $3.12/day, about $95/month |
| Storage | 70 GB at RunPod's $0.10/GB-month rate, about $7/month |
| Credits | about $450 at deploy time, about 4.4 months at this rate |
| Spend to date | the RunPod console Billing page, or the `list-billing` call of the RunPod MCP server |

Metrics: both `llama-server` processes run with `--metrics` on `127.0.0.1`, but the pod runs no Prometheus, Grafana or Phoenix, so nothing collects them. Read GPU memory with `nvidia-smi` (see below); the console GPU reading showed 0 while 5.7 GB was in use.

### Operating the pod

The GPU deploy was one always-on community RTX 3070 pod named `sparky` at $0.13/hr (about $95 a month). It runs whether or not anyone talks to the bot, because the bot holds a Discord gateway connection. Billing stops only when the pod is stopped or terminated.

| Task | How |
|---|---|
| Deploy a change | Merge to `main`; CD publishes `sparkyai-runpod:<sha>` when its inputs change. Set the pod image to that tag. Changing the image restarts the pod. |
| Change a secret | Edit the pod environment in the RunPod console. The pod restarts with the new values. |
| Read logs | Pod logs in the console. Every service writes to the container log; supervisord prefixes its own state changes. |
| Restart | Restart the pod. `start.sh` reruns and supervisord starts every service. |
| Run a one-off command | Set the container start command to `bash -c "<command>; exec /usr/local/bin/sparky-start"`, restart, read the output at the top of the logs, then clear the start command. The pod has no SSH key registered, so this is the way in. |
| Check the GPU | Run `nvidia-smi` as above. The console GPU memory reading is not reliable. |

State on the pod is the GGUF cache under `/workspace/models` and the engine store under `/workspace/sparky`: `sparky.db` and a daily copy in `backups/` (the newest 7 are kept). The index lives on Platform. If the host fails, create a new pod as above, copy the newest backup to `/workspace/sparky/sparky.db`, and start it; the first boot downloads the models again.

### The engine store

The engine keeps conversations, memories and pending confirmations in one SQLite file (`[store]` in `sparky.toml`). Only the engine opens it. `SPARKY_MEMORY__KEY` seals sensitive memories; without the same key they do not open, so keep it with the other pod secrets.

| | compose | RunPod pod |
|---|---|---|
| File | `/data/sparky.db` on the `enginedata` volume | `$SPARKY_DATA_DIR/sparky/sparky.db` |
| Backups | `/data/backups` | `$SPARKY_DATA_DIR/sparky/backups` |

To restore: stop the engine, copy a backup over the file (remove `sparky.db-wal` and `sparky.db-shm` beside it), and start the engine.

To move from Platform once: set `SPARKY_STORE__BACKEND=platform`, deploy, run `engine import-platform` (`just import-platform`) on the host, check the counts it logs, then remove `SPARKY_STORE__BACKEND` and restart. A second run is refused unless `--again` is passed. Turns written between the import and the restart stay on Platform. The token needs `agents:read` and Platform needs the `/api/agents/export/*` routes.

### Troubleshooting

Start with the pod logs. Search for `exited:`, `gave up` and `FATAL` to find the failing program; supervisord names it (`chat`, `embed`, `engine`, `discord`), and the lines just before carry its own error.

| Symptom in the logs or Discord | Cause | Fix |
|---|---|---|
| Bot shows offline; `discord` exits at boot | Bad or rotated `SPARKY_DISCORD__TOKEN`, or wrong `SPARKY_DISCORD__GUILD_ID` | Correct the pod env. A healthy start logs `connected` with the guild id, then `commands registered`. |
| Bot is online but never answers | `chat` or `embed` is not running | Find their exit reason below. The engine logs `could not read the chat server context` once at boot while the models load; that alone is harmless. |
| `chat`/`embed` exit 127, `libllama-server-impl.so: cannot open shared object file` | `/app` is not on the library path | Use an image built after PR #62, or add `LD_LIBRARY_PATH=/app` to the pod env. |
| `chat`/`embed` log `GET failed (429)` from the Hub | No `HF_TOKEN`; the community host shares a rate-limited IP | Set `HF_TOKEN` to a read token. Downloads land in `/workspace/models` and are reused after that. |
| `chat`/`embed` log `out of memory` or a CUDA allocation error | The models do not fit in 8 GB | Lower `SPARKY_CHAT_CTX`, pick a smaller GGUF, or move to a larger GPU (see GPU deployment). |
| Answers are very slow; `nvidia-smi` shows no `llama-server` processes | The models run on the CPU | Recreate the pod with a host CUDA of 12.8 or newer. |
| `engine` exits status 1 at boot | `Config::validate` rejected the configuration, or Platform is unreachable | The line before the exit names the field or the failed Platform call. Fix the pod env or `sparky.toml` and ship a new image. |
| Pod is gone, or the console says the machine is unavailable | The community host left | Create a new pod (see Operating the pod). |
| Pod stuck pulling the image, or `unauthorized` | The GHCR package is private, or the tag does not exist | Make `sparkyai-runpod` public in the GitHub package settings; check the tag in the CD run. |
| CD `image (runpod)` job fails | A build input moved upstream (the llama.cpp server image, for example) | Read the failing step in the Actions log and pin or replace that input in `runpod.Dockerfile`. |

These lines appear on every healthy boot and need no action: the llama.cpp `control-looking token` warning and `phoenix url is unset`.

## Models

Two `llama-server` containers: chat `:8000` and embeddings `:8001`. `just model` uses the CUDA image and reserves an NVIDIA device, so it needs a GPU and the container toolkit. Without one:

- `just model-cpu` applies `deploy/compose.cpu.yml`: the plain image, no device reservation, the same GGUFs on the processor. Set `SPARKY_CHAT_NGL=0` and `SPARKY_EMBED_NGL=0`.
- Or point `SPARKY_MODEL__BASE_URL` at any OpenAI-compatible endpoint, with its API key.

Details: `deploy/inference/README.md`.

## Web

`apps/web` builds to static files (`npm run build` to `apps/web/dist`). Deploy to Vercel (root directory `apps/web`) or any static host. It is not part of a Docker image.

The `/chat`, `/memories` and `/admin` pages call the engine from the browser. On Vercel the site serves the engine under its own origin: `api/engine.ts` is an edge function that forwards `/api/engine/*` to `ENGINE_URL` and streams the answer back, so the session cookie is first-party and browsers that block third-party cookies still sign in. Without `VITE_ENGINE_URL` the pages say the chat is not set up.

Set up, once:

1. Discord application (the bot's): under OAuth2, add the redirect `https://<site>/api/engine/web/callback`, and copy the client id and secret.
2. Engine `.env` on the pod: `SPARKY_WEB__ENABLED=true`, `SPARKY_WEB__DISCORD_CLIENT_ID`, `SPARKY_WEB__DISCORD_CLIENT_SECRET`, `SPARKY_WEB__SESSION_KEY` (generate with `openssl rand -base64 32`), `SPARKY_WEB__PUBLIC_URL=https://<site>/api/engine`, `SPARKY_WEB__SITE_URL=https://<site>`, and `SPARKY_HTTP__CORS_ORIGINS='["https://<site>"]'`. Restart the engine.
3. Vercel project (root directory `apps/web`): `VITE_ENGINE_URL=/api/engine` and `ENGINE_URL=<engine https origin>` (on RunPod, the pod's proxy URL for the engine port). Redeploy, since `VITE_ENGINE_URL` is read at build time.
4. Check: open `https://<site>/chat`, sign in, and send a message. A moderator (Manage Server) also sees the Admin link.

A cookie counts when the request `Origin` is the site, or, with no `Origin`, when the browser marks it `Sec-Fetch-Site: same-origin`. A host without the proxy can set `VITE_ENGINE_URL` to the engine origin and `SPARKY_WEB__PUBLIC_URL` to it instead; the cookie is then third-party and some browsers drop it. `apps/web/vercel.json` sends `/api/engine/*` to the function and every other path without a file extension to `index.html`.

## The sandbox

`run_sandbox` needs a container runtime. Compose provides `sandboxd`, a `docker:27-dind` daemon with no host port; the engine reaches it over `DOCKER_HOST=tcp://sandboxd:2375`, and the engine image carries only the client. On a developer host running `just engine`, the local `docker` serves.

Commands run in `ghcr.io/theaisocietyasu/sparkyai-sandbox` (`deploy/docker/sandbox.Dockerfile`) with a read-only root, dropped capabilities, a non-root user, and a `noexec` workspace, so `python3 script.py` runs but `./script` does not. With `sandbox.egress` off a container has no network; with it on, it reaches public HTTP and HTTPS only through `sparkyai-sandbox-proxy` (`deploy/sandbox/squid.conf`). `just sandbox-images` builds both images locally; `just sandbox-down` removes the containers the engine started.

The runtime is probed at boot. `sandbox.required = true` refuses to start without one; `false` leaves `run_sandbox` unregistered.

With `sandbox.runtime = "wasm"` no container runtime is needed. The engine runs bash, python3 (standard library only), grep, sed, rg and the coreutils as WebAssembly in a child of itself, with no network and one kept folder per student. `deploy/wasm/fetch.sh deploy/wasm/packages.txt <folder>` downloads the pinned packages and checks each sha256; point `sandbox.wasm_packages` at that folder. The first run compiles them (about a minute); later runs take about a second.

## Images

On push to `main`, CD builds and pushes `ghcr.io/theaisocietyasu/sparkyai-rust`, `sparkyai-sandbox`, `sparkyai-sandbox-proxy` and `sparkyai-runpod`, tagged `<sha>` and `main`, only for images whose inputs changed (`workflow_dispatch` rebuilds all).

## Observability

- Traces, LLM generations, product events: self-hosted Phoenix, below. Every app exports OpenTelemetry to `SPARKY_TELEMETRY__PHOENIX_URL` (compose sets it from `SPARKY_PHOENIX_URL`); an empty URL disables export. Spans land in the project named by `telemetry.project_name`. A Discord conversation is one `session.id`, and each product event is its own span.
- Logs: pretty in development, JSON to stdout otherwise. The developer console also writes `.sparky/logs/`; deployed logs stay with the platform log driver.
- Metrics: `just metrics` starts Prometheus (:9090) and Grafana (:3000, dashboard **SparkyAI inference**), both on loopback. They scrape `llama-server` (`chat` and `embed` run with `--metrics`). On a GPU host `just gpu-metrics` adds utilisation, VRAM and temperature panels; the exporter runs under the nvidia container runtime with the `utility` driver capability.

Phoenix holds one `llm` span per model call (full prompt, full reply, token counts, latency) inside a trace tree per conversation; [loupe](https://github.com/ashworks1706/loupe) reads these spans for fine-tuning. Prometheus holds server-side time series: throughput, queue depth, batching.

### Phoenix

```bash
just phoenix       # trace UI on http://localhost:6006, loopback
```

One container, `arizephoenix/phoenix:version-20.11.0`, data in the `phoenixdata` volume, UI and OTLP endpoint on one port. Export is off until `SPARKY_TELEMETRY__PHOENIX_URL=http://localhost:6006` is in `.env`. For the compose apps set `SPARKY_PHOENIX_URL=http://phoenix:6006`. Phoenix has no authentication by default and holds full prompts and replies, so in production it has no host port. When it does authenticate, set `SPARKY_TELEMETRY__PHOENIX_API_KEY`; every app sends it as a bearer token, and so does `loupe data export` in [loupe](https://github.com/ashworks1706/loupe), which reads `llm` spans through `GET /v1/projects/<project>/spans`.

### Reading the dashboard

| Panel | Metric | Reads as |
|---|---|---|
| Generation throughput | `rate(llamacpp:tokens_predicted_total[1m])` | tokens per wall-clock second, all requests combined |
| Prompt throughput | `llamacpp:prompt_tokens_total`, `..._cached_total` | prompt tokens evaluated vs. reused from cache |
| Queue | `llamacpp:requests_processing`, `..._deferred` | deferred above zero means requests are waiting for a slot |
| Batching efficiency | `llamacpp:n_busy_slots_per_decode` | near 1 with a non-empty queue means `--parallel` is too low |
| Prompt cache hit rate | cached / (cached + new) | a stable system prompt keeps this high |
| Server-reported speed | `llamacpp:predicted_tokens_seconds`, `..._prompt_tokens_seconds` | llama-server's own running average, independent of load |
| Context ceiling | `llamacpp:n_tokens_max` | the largest context one slot accepts |

Both servers start with two slots (`SPARKY_CHAT_PARALLEL`, `SPARKY_EMBED_PARALLEL`). Each slot takes a `--ctx-size / N` share of the context window, so raise both together.

Grafana's admin password comes from `SPARKY_GRAFANA_PASSWORD` (compose default `admin`). Both ports bind to `127.0.0.1`; set a real password before exposing either.
