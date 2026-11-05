#!/usr/bin/env bash
# Prepares the model cache and the store folder, then runs supervisord. The pod runs the two model
# servers, the engine and the bot. The engine SQLite file and its backups are on the volume. With SPARKY_POD_ROLE=models only the
# two model servers run, for an engine and bot that run elsewhere. With SPARKY_POD_ROLE=apps only the engine and the bot run,
# against the model API in SPARKY_MODEL__BASE_URL, on a pod with no GPU.
set -euo pipefail

data="${SPARKY_DATA_DIR:-/workspace}"

if [ -n "${SPARKY_MODELS_API_KEY:-}" ]; then
    export SPARKY_MODELS_HOST=0.0.0.0
    export SPARKY_MODEL__API_KEY="$SPARKY_MODELS_API_KEY"
else
    export SPARKY_MODELS_HOST=127.0.0.1
fi

export SPARKY_RUN_APPS=true SPARKY_RUN_MODELS=true
case "${SPARKY_POD_ROLE:-all}" in
    all) ;;
    models)
        if [ -z "${SPARKY_MODELS_API_KEY:-}" ]; then
            echo "SPARKY_POD_ROLE=models needs SPARKY_MODELS_API_KEY" >&2
            exit 1
        fi
        export SPARKY_RUN_APPS=false
        ;;
    apps)
        case "${SPARKY_MODEL__BASE_URL:-}" in
            http://127.0.0.1*|"")
                echo "SPARKY_POD_ROLE=apps needs SPARKY_MODEL__BASE_URL set to a model API" >&2
                exit 1
                ;;
        esac
        export SPARKY_RUN_MODELS=false
        ;;
    *)
        echo "SPARKY_POD_ROLE must be all, models or apps" >&2
        exit 1
        ;;
esac

# Phoenix with sign-in on, its database and keys on the volume. The keys are made once; the admin
# key is the bearer token the engine and the bot export spans with. Off unless SPARKY_RUN_PHOENIX=true.
export SPARKY_RUN_PHOENIX="${SPARKY_RUN_PHOENIX:-false}"
if [ "$SPARKY_RUN_PHOENIX" = "true" ]; then
    phoenix="$data/phoenix"
    mkdir -p "$phoenix"
    if [ ! -f "$phoenix/keys.env" ]; then
        key() { head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
        (umask 077 && printf 'PHOENIX_SECRET=%s\nPHOENIX_ADMIN_SECRET=%s\n' "$(key)" "$(key)" > "$phoenix/keys.env")
    fi
    set -a
    # shellcheck disable=SC1091
    . "$phoenix/keys.env"
    set +a
    ui="${SPARKY_TELEMETRY__PHOENIX_UI_URL:-https://${RUNPOD_POD_ID}-6006.proxy.runpod.net}"
    export PHOENIX_WORKING_DIR="$phoenix" PHOENIX_ENABLE_AUTH=true PHOENIX_HOST=0.0.0.0 PHOENIX_PORT=6006 \
        PHOENIX_USE_SECURE_COOKIES=true PHOENIX_CSRF_TRUSTED_ORIGINS="$ui" PHOENIX_TELEMETRY_ENABLED=false \
        PHOENIX_DEFAULT_RETENTION_POLICY_DAYS="${SPARKY_PHOENIX_RETENTION_DAYS:-30}"
    export SPARKY_TELEMETRY__PHOENIX_URL="${SPARKY_TELEMETRY__PHOENIX_URL:-http://127.0.0.1:6006}"
    export SPARKY_TELEMETRY__PHOENIX_API_KEY="${SPARKY_TELEMETRY__PHOENIX_API_KEY:-$PHOENIX_ADMIN_SECRET}"
    export SPARKY_TELEMETRY__PHOENIX_UI_URL="$ui"
fi

mkdir -p "$data/models" "$data/sparky/backups"
export SPARKY_STORE__PATH="${SPARKY_STORE__PATH:-$data/sparky/sparky.db}"
export SPARKY_STORE__BACKUP_DIR="${SPARKY_STORE__BACKUP_DIR:-$data/sparky/backups}"
exec /usr/bin/supervisord -c /etc/sparky/supervisord.conf
