#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# PrivStack Headless Server — Dev Launcher
#
# Usage:
#   ./run-server.sh --with-plugins                 # Build plugins + run
#   ./run-server.sh --with-plugins --workspace "X"  # Build plugins + specific workspace
#   ./run-server.sh                                # Run (plugins must already be built)
#   ./run-server.sh --live                         # Use live data dir (not test-data)
#   ./run-server.sh --build                        # Rebuild server before running
#   ./run-server.sh --port 8080                    # Override port
#   ./run-server.sh --setup                        # Run setup wizard
# ============================================================================

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
SERVER_DIR="$REPO_ROOT/desktop/PrivStack.Server"
PLUGINS_SRC_DIR="$(cd "$REPO_ROOT/.." && pwd)/PrivStack-Plugins"
PLUGINS_OUTPUT_DIR="$REPO_ROOT/plugins"
TEST_DATA_DIR="$REPO_ROOT/test-data"
BIN_DIR="$SERVER_DIR/bin/Release/net9.0"

BUILD=false
WITH_PLUGINS=false
USE_LIVE=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build)           BUILD=true; shift ;;
        --with-plugins)    WITH_PLUGINS=true; BUILD=true; shift ;;
        --live)            USE_LIVE=true; shift ;;
        --workspace)       EXTRA_ARGS+=("--workspace" "$2"); shift 2 ;;
        --port)            EXTRA_ARGS+=("--port" "$2"); shift 2 ;;
        --bind)            EXTRA_ARGS+=("--bind" "$2"); shift 2 ;;
        --setup)           EXTRA_ARGS+=("--setup"); shift ;;
        --setup-tls)       EXTRA_ARGS+=("--setup-tls"); shift ;;
        --setup-network)   EXTRA_ARGS+=("--setup-network"); shift ;;
        --setup-policy)    EXTRA_ARGS+=("--setup-policy"); shift ;;
        --show-api-key)    EXTRA_ARGS+=("--show-api-key"); shift ;;
        --generate-api-key) EXTRA_ARGS+=("--generate-api-key"); shift ;;
        *)                 echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# Build server if requested or binary doesn't exist
if [ "$BUILD" = true ] || [ ! -f "$BIN_DIR/privstack-server" ]; then
    echo "==> Building privstack-server (Release)..."
    dotnet build "$SERVER_DIR" -c Release --no-incremental --nologo -v quiet
    echo "==> Server build complete."
fi

# Build plugins if requested
if [ "$WITH_PLUGINS" = true ]; then
    if [ ! -d "$PLUGINS_SRC_DIR" ]; then
        echo "ERROR: Plugin source not found at $PLUGINS_SRC_DIR" >&2
        echo "Expected sibling directory: ../PrivStack-Plugins/" >&2
        exit 1
    fi

    echo "==> Building plugins..."
    plugin_count=0
    headless_ids=()

    # Phase 1: Build .Headless projects first (Avalonia-free, preferred by server)
    for plugin_csproj in "$PLUGINS_SRC_DIR"/PrivStack.Plugin.*.Headless/PrivStack.Plugin.*.Headless.csproj; do
        [ -f "$plugin_csproj" ] || continue
        plugin_name=$(basename "${plugin_csproj%.csproj}")

        plugin_out="$PLUGINS_OUTPUT_DIR/$plugin_name"
        plugin_dll="$plugin_out/$plugin_name.dll"

        # Track which base plugin IDs have headless variants
        # e.g., "PrivStack.Plugin.Tasks.Headless" → "PrivStack.Plugin.Tasks"
        base_name="${plugin_name%.Headless}"
        headless_ids+=("$base_name")

        # Incremental check
        if [ -f "$plugin_dll" ]; then
            plugin_dir=$(dirname "$plugin_csproj")
            needs_rebuild=false
            while IFS= read -r -d '' src_file; do
                if [ "$src_file" -nt "$plugin_dll" ]; then
                    needs_rebuild=true
                    break
                fi
            done < <(find "$plugin_dir" \( -name "*.cs" -o -name "*.csproj" \) -print0 2>/dev/null)

            if [ "$needs_rebuild" = false ]; then
                plugin_count=$((plugin_count + 1))
                continue
            fi
        fi

        echo "    Building $plugin_name (headless)..."
        dotnet publish "$plugin_csproj" -c Release -o "$plugin_out" --nologo -v quiet
        plugin_count=$((plugin_count + 1))
    done

    # Phase 2: Build remaining plugins (skip full plugins that have a .Headless variant)
    for plugin_csproj in "$PLUGINS_SRC_DIR"/PrivStack.Plugin.*/PrivStack.Plugin.*.csproj; do
        [ -f "$plugin_csproj" ] || continue
        plugin_name=$(basename "${plugin_csproj%.csproj}")

        # Skip test projects and headless projects (already built above)
        case "$plugin_name" in
            *.Tests|*.TestRunner|*.Headless) continue ;;
        esac

        # Skip full plugins that have a .Headless variant (server doesn't need them)
        skip=false
        for hid in "${headless_ids[@]+"${headless_ids[@]}"}"; do
            if [ "$plugin_name" = "$hid" ]; then
                skip=true
                break
            fi
        done
        if [ "$skip" = true ]; then
            _log_skip="${plugin_name} (headless variant used instead)"
            continue
        fi

        plugin_out="$PLUGINS_OUTPUT_DIR/$plugin_name"
        plugin_dll="$plugin_out/$plugin_name.dll"

        # Incremental check
        if [ -f "$plugin_dll" ]; then
            plugin_dir=$(dirname "$plugin_csproj")
            needs_rebuild=false
            while IFS= read -r -d '' src_file; do
                if [ "$src_file" -nt "$plugin_dll" ]; then
                    needs_rebuild=true
                    break
                fi
            done < <(find "$plugin_dir" \( -name "*.cs" -o -name "*.csproj" -o -name "*.axaml" \) -print0 2>/dev/null)

            if [ "$needs_rebuild" = false ]; then
                plugin_count=$((plugin_count + 1))
                continue
            fi
        fi

        echo "    Building $plugin_name..."
        dotnet publish "$plugin_csproj" -c Release -o "$plugin_out" --nologo -v quiet
        plugin_count=$((plugin_count + 1))
    done

    echo "==> $plugin_count plugins ready in plugins/"
fi

# Check that plugins directory exists (warn if not)
if [ ! -d "$PLUGINS_OUTPUT_DIR" ]; then
    echo "WARNING: No plugins/ directory found. Run with --with-plugins to build them." >&2
    echo "The server will start but won't have any API routes." >&2
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
