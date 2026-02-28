#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# PrivStack Headless Server — Dev Launcher
#
# Usage:
#   ./run-server.sh                          # Interactive workspace picker
#   ./run-server.sh --workspace "Steve@Home" # Use specific workspace
#   ./run-server.sh --live                   # Use live data dir (not test-data)
#   ./run-server.sh --build                  # Build before running
#   ./run-server.sh --port 8080              # Override port
#   ./run-server.sh --setup                  # Run setup wizard
# ============================================================================

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
SERVER_DIR="$REPO_ROOT/desktop/PrivStack.Server"
TEST_DATA_DIR="$REPO_ROOT/test-data"
BIN_DIR="$SERVER_DIR/bin/Release/net9.0"

BUILD=false
USE_LIVE=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build)        BUILD=true; shift ;;
        --live)         USE_LIVE=true; shift ;;
        --workspace)    EXTRA_ARGS+=("--workspace" "$2"); shift 2 ;;
        --port)         EXTRA_ARGS+=("--port" "$2"); shift 2 ;;
        --bind)         EXTRA_ARGS+=("--bind" "$2"); shift 2 ;;
        --setup)        EXTRA_ARGS+=("--setup"); shift ;;
        --setup-tls)    EXTRA_ARGS+=("--setup-tls"); shift ;;
        --setup-network) EXTRA_ARGS+=("--setup-network"); shift ;;
        --setup-policy) EXTRA_ARGS+=("--setup-policy"); shift ;;
        --show-api-key) EXTRA_ARGS+=("--show-api-key"); shift ;;
        --generate-api-key) EXTRA_ARGS+=("--generate-api-key"); shift ;;
        *)              echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# Build if requested or binary doesn't exist
if [ "$BUILD" = true ] || [ ! -f "$BIN_DIR/privstack-server" ]; then
    echo "==> Building privstack-server (Release)..."
    dotnet build "$SERVER_DIR" -c Release --no-incremental -v quiet
    echo "==> Build complete."
fi

# Password — prompt if not set
if [ -z "${PRIVSTACK_MASTER_PASSWORD:-}" ]; then
    echo -n "[privstack] Master password: " >&2
    read -rs PRIVSTACK_MASTER_PASSWORD
    echo >&2
fi
export PRIVSTACK_MASTER_PASSWORD

# Data directory — test-data by default, live with --live
if [ "$USE_LIVE" = false ]; then
    if [ ! -d "$TEST_DATA_DIR" ]; then
        echo "No test-data/ directory found. Run build.sh --run --with-plugins first," >&2
        echo "or use --live for the system data directory." >&2
        exit 1
    fi
    export PRIVSTACK_DATA_DIR="$TEST_DATA_DIR"
    echo "[privstack] Data: $TEST_DATA_DIR"
else
    echo "[privstack] Data: system default"
fi

exec "$BIN_DIR/privstack-server" "${EXTRA_ARGS[@]}"
