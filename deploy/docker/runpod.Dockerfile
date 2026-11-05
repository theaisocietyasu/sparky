# syntax=docker/dockerfile:1.7
# Sparky in one RunPod GPU pod: the chat and embed models, the engine and the bot. The stores
# live on the platform. RunPod pods cannot run containers, so supervisord runs every service as
# a process and run_sandbox runs WebAssembly inside the engine. The model cache, the compiled
# packages and the student workspaces live under /workspace, the pod volume.
ARG LLAMA_IMAGE=ghcr.io/ggml-org/llama.cpp:server-cuda
ARG PHOENIX_VERSION=21.0.0

FROM mirror.gcr.io/library/rust:1.96-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p engine -p discord

# llama.cpp server image: Ubuntu 24.04 with the CUDA runtime and /app/llama-server.
FROM ${LLAMA_IMAGE}
ARG PHOENIX_VERSION
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl supervisor python3-venv \
    && rm -rf /var/lib/apt/lists/*
# Phoenix, the trace viewer. It runs only with SPARKY_RUN_PHOENIX=true.
RUN python3 -m venv /opt/phoenix \
    && /opt/phoenix/bin/pip install --no-cache-dir "arize-phoenix==${PHOENIX_VERSION}"

COPY --from=builder /app/target/release/engine /usr/local/bin/engine
COPY --from=builder /app/target/release/discord /usr/local/bin/discord
COPY sparky.toml /etc/sparky/sparky.toml
COPY deploy/runpod/supervisord.conf /etc/sparky/supervisord.conf
COPY deploy/runpod/start.sh /usr/local/bin/sparky-start
COPY deploy/wasm /tmp/wasm
RUN /tmp/wasm/fetch.sh /tmp/wasm/packages.txt /opt/sparky/wasm && rm -rf /tmp/wasm

# Addresses inside the pod. Secrets come from the pod environment.
# llama-server links its shared libraries from /app.
ENV LD_LIBRARY_PATH=/app:${LD_LIBRARY_PATH}
ENV SPARKY_CONFIG_FILE=/etc/sparky/sparky.toml \
    SPARKY_DATA_DIR=/workspace \
    LLAMA_CACHE=/workspace/models \
    SPARKY_APP__HTTP_ADDR=127.0.0.1:8080 \
    SPARKY_ENGINE__BASE_URL=http://127.0.0.1:8080 \
    SPARKY_MODEL__BASE_URL=http://127.0.0.1:8000/v1 \
    SPARKY_MODEL__API_KEY=local \
    SPARKY_SANDBOX__ENABLED=true \
    SPARKY_SANDBOX__REQUIRED=false \
    SPARKY_SANDBOX__RUNTIME=wasm \
    SPARKY_SANDBOX__EGRESS=false \
    SPARKY_SANDBOX__WASM_PACKAGES=/opt/sparky/wasm \
    SPARKY_SANDBOX__WASM_CACHE=/workspace/sandbox/cache \
    SPARKY_SANDBOX__WASM_WORK=/workspace/sandbox/work \
    SPARKY_PROMPT__SANDBOX_RETRY_LINE= \
    SPARKY_CHAT_GGUF=unsloth/Qwen3.5-9B-GGUF:Q4_K_M \
    SPARKY_CHAT_CTX=16384 \
    SPARKY_CHAT_PARALLEL=2 \
    SPARKY_MODEL__NAME=unsloth/Qwen3.5-9B-GGUF:Q4_K_M \
    SPARKY_EMBED_GGUF=Qwen/Qwen3-Embedding-0.6B-GGUF:Q8_0 \
    SPARKY_EMBED_CTX=4096 \
    SPARKY_EMBED_PARALLEL=2
WORKDIR /
ENTRYPOINT ["/usr/local/bin/sparky-start"]
