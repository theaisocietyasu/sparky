#!/usr/bin/env bash
# Prepares the model cache and the store folder, then runs supervisord. The pod runs the two model
# servers, the engine and the bot. The engine SQLite file and its backups are on the volume. With SPARKY_POD_ROLE=models only the
# two model servers run, for an engine and bot that run elsewhere.
set -euo pipefail

data="${SPARKY_DATA_DIR:-/workspace}"

if [ -n "${SPARKY_MODELS_API_KEY:-}" ]; then
    export SPARKY_MODELS_HOST=0.0.0.0
    export SPARKY_MODEL__API_KEY="$SPARKY_MODELS_API_KEY" SPARKY_EMBEDDING__API_KEY="$SPARKY_MODELS_API_KEY"
else
    export SPARKY_MODELS_HOST=127.0.0.1
fi

export SPARKY_RUN_APPS=true
if [ "${SPARKY_POD_ROLE:-all}" = "models" ]; then
    if [ -z "${SPARKY_MODELS_API_KEY:-}" ]; then
        echo "SPARKY_POD_ROLE=models needs SPARKY_MODELS_API_KEY" >&2
        exit 1
    fi
    export SPARKY_RUN_APPS=false
fi

mkdir -p "$data/models" "$data/sparky/backups"
export SPARKY_STORE__PATH="${SPARKY_STORE__PATH:-$data/sparky/sparky.db}"
export SPARKY_STORE__BACKUP_DIR="${SPARKY_STORE__BACKUP_DIR:-$data/sparky/backups}"
exec /usr/bin/supervisord -c /etc/sparky/supervisord.conf
