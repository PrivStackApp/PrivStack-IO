#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# PrivStack Dev Build — Core (Rust FFI) + Desktop (.NET) + Plugins (optional)
#
# Usage:
#   ./build.sh                         # Build core + desktop (debug)
#   ./build.sh --release               # Build core + desktop (release)
#   ./build.sh --run                   # Just launch (no build)
#   ./build.sh --run --rebuild         # Build everything, then launch
#   ./build.sh --run --with-plugins    # Incremental build + plugins, then launch
#   ./build.sh --skip-core             # Build desktop only (use existing native lib)
#   ./build.sh --skip-desktop          # Build core only
#   ./build.sh --test                  # Build + run all tests
#   ./build.sh --clean                 # Wipe artifacts, then full build
#   ./build.sh --fresh                 # Wipe DB/settings, then build + launch
#   ./build.sh --clean-plugins         # Remove plugins/ test directory
#   ./build.sh --debug                 # Build with verbose (debug) output
#   ./build.sh --warn                  # Build with warnings and errors only
#
# Log levels: --debug (all), --info (default), --warn, --error
# --with-plugins builds all plugins from ../PrivStack-Plugins/ into plugins/
# so the app auto-discovers them at launch (via dev-time fallback path).
# ============================================================================

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
CORE_DIR="$REPO_ROOT/core"
DESKTOP_DIR="$REPO_ROOT/desktop"

# Defaults
MODE="debug"
SKIP_CORE=false
SKIP_DESKTOP=false
RUN_AFTER=false
RUN_TESTS=false
FRESH_DB=false
CLEAN=false
REBUILD=false
WITH_PLUGINS=false
CLEAN_PLUGINS=false
PERSIST_TEST_DATA=false
LOG_LEVEL=2  # 0=error, 1=warn, 2=info, 3=debug

log_error() { echo "$@"; }
log_warn()  { [ "$LOG_LEVEL" -ge 1 ] && echo "$@" || true; }
log_info()  { [ "$LOG_LEVEL" -ge 2 ] && echo "$@" || true; }
log_debug() { [ "$LOG_LEVEL" -ge 3 ] && echo "$@" || true; }

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build the PrivStack core (Rust FFI) and desktop (.NET) projects.

Options:
  --release          Build in release mode (default: debug)
  --skip-core        Skip the Rust core build
  --skip-desktop     Skip the .NET desktop build
  --run              Launch the desktop app (skips build unless --rebuild or --with-plugins)
  --rebuild          Force full rebuild (ignore incremental caches)
  --with-plugins     Incrementally build changed plugins into plugins/ for integrated testing
  --clean-plugins    Remove the plugins/ test directory
  --test             Run tests after building (starts test containers, tears down after)
  --persist          Keep test containers and data running after --test for manual audit
  --clean            Wipe all build artifacts before building
  --fresh            Wipe all local databases and settings, start clean
  --debug            Log level: show all output (debug + info + warn + error)
  --info             Log level: show info + warn + error (default)
  --warn             Log level: show warn + error only
  --error            Log level: show errors only
  -h, --help         Show this help

Examples:
  ./build.sh                       # Full build (core + desktop, debug)
  ./build.sh --release             # Full build (release)
  ./build.sh --run                 # Just launch (no build)
  ./build.sh --run --rebuild       # Build everything, then launch
  ./build.sh --skip-core           # Desktop only (reuse existing native lib)
  ./build.sh --skip-core --run     # Rebuild desktop, then launch
  ./build.sh --run --with-plugins  # Incremental build + launch with plugins
  ./build.sh --run --with-plugins --rebuild  # Force full rebuild + launch
  ./build.sh --test                # Build + run all tests (containers auto-teardown)
  ./build.sh --test --persist      # Build + run tests, keep MinIO/MySQL running for audit
  ./build.sh --clean --run         # Nuke artifacts, rebuild, launch
  ./build.sh --fresh --run         # Nuke DB, rebuild, launch fresh
  ./build.sh --clean-plugins       # Remove plugins/ test directory
EOF
    exit 0
}

while [ $# -gt 0 ]; do
    case "$1" in
        --release)       MODE="release"; shift ;;
        --skip-core)     SKIP_CORE=true; shift ;;
        --skip-desktop)  SKIP_DESKTOP=true; shift ;;
        --run)           RUN_AFTER=true; shift ;;
        --rebuild)       REBUILD=true; shift ;;
        --with-plugins)  WITH_PLUGINS=true; shift ;;
        --clean-plugins) CLEAN_PLUGINS=true; shift ;;
        --test)          RUN_TESTS=true; shift ;;
        --persist)       PERSIST_TEST_DATA=true; shift ;;
        --clean)         CLEAN=true; shift ;;
        --fresh)         FRESH_DB=true; shift ;;
        --debug)         LOG_LEVEL=3; shift ;;
        --info)          LOG_LEVEL=2; shift ;;
        --warn)          LOG_LEVEL=1; shift ;;
        --error)         LOG_LEVEL=0; shift ;;
        -h|--help)       usage ;;
        *) log_error "Unknown option: $1"; usage ;;
    esac
done

# --run alone (no --rebuild, --clean, --fresh, --test, --with-plugins) = skip all builds, just launch
# --run --with-plugins = incremental build (cargo/dotnet handle this natively)
if [ "$RUN_AFTER" = true ] && [ "$REBUILD" = false ] && \
   [ "$CLEAN" = false ] && [ "$FRESH_DB" = false ] && [ "$RUN_TESTS" = false ] && \
   [ "$WITH_PLUGINS" = false ]; then
    SKIP_CORE=true
    SKIP_DESKTOP=true
fi

# Build config
CARGO_PROFILE_FLAG=""
CARGO_TARGET_DIR="debug"
DOTNET_CONFIG="Debug"
if [ "$MODE" = "release" ]; then
    CARGO_PROFILE_FLAG="--release"
    CARGO_TARGET_DIR="release"
    DOTNET_CONFIG="Release"
fi

# Native library name (platform-dependent)
case "$(uname -s)" in
    Darwin)             LIB_NAME="libprivstack_ffi.dylib" ;;
    Linux)              LIB_NAME="libprivstack_ffi.so" ;;
    MINGW*|MSYS*|CYGWIN*) LIB_NAME="privstack_ffi.dll" ;;
    *) log_error "Unsupported OS: $(uname -s)"; exit 1 ;;
esac

# ── Step 0a: Clean ──────────────────────────────────────────────
if [ "$CLEAN" = true ]; then
    log_info "==> Cleaning build artifacts..."

    if [ -d "$CORE_DIR/target" ]; then
        log_debug "    Cleaning Rust core (cargo clean)..."
        cargo clean --manifest-path "$CORE_DIR/Cargo.toml" 2>/dev/null || true
    fi

    log_debug "    Cleaning .NET bin/obj..."
    find "$DESKTOP_DIR" -type d \( -name bin -o -name obj \) -exec rm -rf {} + 2>/dev/null || true

    log_debug "    Clean complete."
fi

# ── Step 0b: Fresh DB ──────────────────────────────────────────
if [ "$FRESH_DB" = true ]; then
    case "$(uname -s)" in
        Darwin)             DATA_DIR="$HOME/Library/Application Support/PrivStack" ;;
        Linux)              DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/PrivStack" ;;
        MINGW*|MSYS*|CYGWIN*) DATA_DIR="$LOCALAPPDATA/PrivStack" ;;
        *) log_error "Unsupported OS for --fresh"; exit 1 ;;
    esac

    if [ -d "$DATA_DIR" ]; then
        log_info "==> Wiping PrivStack data directory: $DATA_DIR"
        if [ "$LOG_LEVEL" -ge 3 ]; then
            ls -la "$DATA_DIR"/ 2>/dev/null || true
            if [ -d "$DATA_DIR/workspaces" ]; then
                log_debug "    Workspaces:"
                ls -la "$DATA_DIR/workspaces"/ 2>/dev/null || true
            fi
        fi
        rm -rf "$DATA_DIR"
        log_debug "    Data directory removed."
    else
        log_info "==> No data directory found at $DATA_DIR (already clean)."
    fi
fi

# ── Step 1: Build Rust core (FFI) ──────────────────────────────
if [ "$SKIP_CORE" = false ]; then
    log_info "==> Building Rust core (privstack-ffi) [$MODE]..."
    cargo build -p privstack-ffi $CARGO_PROFILE_FLAG --manifest-path "$CORE_DIR/Cargo.toml"

    LIB_PATH="$CORE_DIR/target/$CARGO_TARGET_DIR/$LIB_NAME"
    if [ ! -f "$LIB_PATH" ]; then
        log_error "ERROR: Native library not found at $LIB_PATH"
        exit 1
    fi

    # Show size
    if command -v du >/dev/null 2>&1; then
        log_debug "    Native library: $LIB_PATH ($(du -h "$LIB_PATH" | cut -f1))"
    else
        log_debug "    Native library: $LIB_PATH"
    fi
fi

# ── Step 1b: Ensure native lib is where .NET expects it ────────
# The .csproj references core/target/release/. For debug builds, copy the
# debug lib there so dotnet build can find it.
if [ "$SKIP_DESKTOP" = false ] && [ "$MODE" = "debug" ]; then
    RUST_RELEASE_DIR="$CORE_DIR/target/release"
    RUST_DEBUG_DIR="$CORE_DIR/target/debug"

    if [ -f "$RUST_DEBUG_DIR/$LIB_NAME" ]; then
        mkdir -p "$RUST_RELEASE_DIR"
        if [ ! -f "$RUST_RELEASE_DIR/$LIB_NAME" ] || \
           [ "$RUST_DEBUG_DIR/$LIB_NAME" -nt "$RUST_RELEASE_DIR/$LIB_NAME" ]; then
            log_debug "    Copying debug native lib to release dir (.csproj expects release path)..."
            cp "$RUST_DEBUG_DIR/$LIB_NAME" "$RUST_RELEASE_DIR/$LIB_NAME"
        fi
    fi
fi

# ── Step 2: Build .NET desktop ─────────────────────────────────
if [ "$SKIP_DESKTOP" = false ]; then
    log_info "==> Building .NET desktop [$DOTNET_CONFIG]..."
    dotnet build "$DESKTOP_DIR/PrivStack.sln" -c "$DOTNET_CONFIG" --nologo -v quiet
    log_debug "    Desktop build complete."
fi

# ── Step 2a: Build bridge (native messaging relay) ────────────
BRIDGE_DIR="$REPO_ROOT/bridge/PrivStack.Bridge"
BRIDGE_CSPROJ="$BRIDGE_DIR/PrivStack.Bridge.csproj"

if [ "$SKIP_DESKTOP" = false ] && [ -f "$BRIDGE_CSPROJ" ]; then
    log_info "==> Building bridge (native messaging relay)..."
    dotnet build "$BRIDGE_CSPROJ" -c "$DOTNET_CONFIG" --nologo -v quiet

    # Copy bridge binary + runtime files next to the desktop app so FindBridgePath() discovers it
    DESKTOP_BIN="$DESKTOP_DIR/PrivStack.Desktop/bin/$DOTNET_CONFIG/net9.0"
    BRIDGE_BIN="$BRIDGE_DIR/bin/$DOTNET_CONFIG/net9.0"

    if [ -d "$DESKTOP_BIN" ] && [ -d "$BRIDGE_BIN" ]; then
        cp "$BRIDGE_BIN"/privstack-bridge* "$DESKTOP_BIN/" 2>/dev/null || true
        log_debug "    Bridge copied to desktop output."
    fi
fi

# ── Step 2b: Clean plugins ────────────────────────────────────
PLUGINS_OUTPUT_DIR="$REPO_ROOT/plugins"

TEST_DATA_DIR="$REPO_ROOT/test-data"

if [ "$CLEAN_PLUGINS" = true ] || [ "$CLEAN" = true ]; then
    if [ -d "$PLUGINS_OUTPUT_DIR" ]; then
        log_info "==> Removing plugins/ test directory..."
        rm -rf "$PLUGINS_OUTPUT_DIR"
        log_debug "    Plugins directory removed."
    else
        log_info "==> No plugins/ directory to clean."
    fi
    if [ -d "$TEST_DATA_DIR" ]; then
        log_info "==> Removing test-data/ directory..."
        rm -rf "$TEST_DATA_DIR"
        log_debug "    Test data directory removed."
    fi
    # If only --clean-plugins was requested, exit
    if [ "$CLEAN_PLUGINS" = true ] && [ "$SKIP_CORE" = true ] && [ "$SKIP_DESKTOP" = true ] && \
       [ "$RUN_AFTER" = false ] && [ "$RUN_TESTS" = false ]; then
        log_info "==> Done."
        exit 0
    fi
fi

# ── Step 2c: Build plugins (incremental) ─────────────────────
if [ "$WITH_PLUGINS" = true ]; then
    PLUGINS_SRC_DIR="$(cd "$REPO_ROOT/.." && pwd)/PrivStack-Plugins"

    if [ ! -d "$PLUGINS_SRC_DIR" ]; then
        log_error "ERROR: PrivStack-Plugins directory not found at $PLUGINS_SRC_DIR"
        exit 1
    fi

    log_info "==> Building plugins into $PLUGINS_OUTPUT_DIR..."
    mkdir -p "$PLUGINS_OUTPUT_DIR"

    PLUGIN_BUILT=0
    PLUGIN_SKIPPED=0
    PLUGIN_FAILED=0

    for plugin_csproj in "$PLUGINS_SRC_DIR"/PrivStack.Plugin.*/PrivStack.Plugin.*.csproj; do
        plugin_name=$(basename "${plugin_csproj%.csproj}")

        # Skip test/runner projects — they are not runtime plugins
        case "$plugin_name" in
            *.Tests|*.TestRunner) continue ;;
        esac

        plugin_dir=$(dirname "$plugin_csproj")
        plugin_out="$PLUGINS_OUTPUT_DIR/$plugin_name"
        plugin_dll="$plugin_out/$plugin_name.dll"

        # Skip unchanged plugins: if the output DLL exists and no source file
        # (.cs, .csproj, .axaml, .xaml) is newer than it, skip the rebuild.
        # Also check shared dependencies (SDK, UI.Adaptive) — if those were
        # rebuilt, the plugin output needs republishing to pick up new DLLs.
        if [ "$REBUILD" = false ] && [ -f "$plugin_dll" ]; then
            NEEDS_BUILD=false

            # Check plugin source files
            while IFS= read -r -d '' src_file; do
                if [ "$src_file" -nt "$plugin_dll" ]; then
                    NEEDS_BUILD=true
                    break
                fi
            done < <(find "$plugin_dir" \( -name "*.cs" -o -name "*.csproj" -o -name "*.axaml" -o -name "*.xaml" \) -not -path "*/bin/*" -not -path "*/obj/*" -print0)

            # Check shared SDK/UI dependencies — if their build output is newer,
            # the plugin needs republishing to pick up the updated DLLs
            if [ "$NEEDS_BUILD" = false ]; then
                for dep_dll in \
                    "$DESKTOP_DIR/PrivStack.Sdk/bin/$DOTNET_CONFIG/net9.0/PrivStack.Sdk.dll" \
                    "$DESKTOP_DIR/PrivStack.UI.Adaptive/bin/$DOTNET_CONFIG/net9.0/PrivStack.UI.Adaptive.dll"; do
                    if [ -f "$dep_dll" ] && [ "$dep_dll" -nt "$plugin_dll" ]; then
                        NEEDS_BUILD=true
                        log_debug "    $plugin_name: dependency $(basename "$dep_dll") is newer, rebuilding..."
                        break
                    fi
                done
            fi

            if [ "$NEEDS_BUILD" = false ]; then
                PLUGIN_SKIPPED=$((PLUGIN_SKIPPED + 1))
                continue
            fi
        fi

        log_debug "    Building $plugin_name..."
        if dotnet publish "$plugin_csproj" -c "$DOTNET_CONFIG" -o "$plugin_out" --nologo -v quiet 2>&1; then
            PLUGIN_BUILT=$((PLUGIN_BUILT + 1))
        else
            log_warn "    WARNING: Failed to build $plugin_name"
            PLUGIN_FAILED=$((PLUGIN_FAILED + 1))
        fi
    done

    log_info "    Plugins — built: $PLUGIN_BUILT, up-to-date: $PLUGIN_SKIPPED, failed: $PLUGIN_FAILED"
    if [ "$PLUGIN_FAILED" -gt 0 ]; then
        log_warn "    WARNING: Some plugins failed to build. Continuing anyway..."
    fi
fi

# ── Step 3: Tests ──────────────────────────────────────────────
if [ "$RUN_TESTS" = true ]; then
    COMPOSE_FILE="$REPO_ROOT/docker-compose.test.yml"
    COMPOSE_UP=false

    # Start test containers if compose file exists
    if [ -f "$COMPOSE_FILE" ]; then
        log_info "==> Starting test containers (MinIO + MySQL)..."

        # Kill any processes squatting on our required ports (9000=MinIO, 3307=MySQL)
        for PORT in 9000 3307; do
            PID=$(lsof -ti :"$PORT" 2>/dev/null || true)
            if [ -n "$PID" ]; then
                log_warn "    Port $PORT in use by PID $PID — killing..."
                kill -9 $PID 2>/dev/null || sudo kill -9 $PID 2>/dev/null || true
                sleep 0.5
            fi
        done

        # Start persistent services first, then run the init container separately.
        # minio-setup exits after creating the bucket, which causes --wait to
        # return non-zero and trip set -e.
        docker compose -f "$COMPOSE_FILE" up -d --wait minio mysql
        docker compose -f "$COMPOSE_FILE" run --rm minio-setup
        COMPOSE_UP=true
    fi

    TEST_EXIT=0

    log_info "==> Running Rust tests..."
    cargo test --manifest-path "$CORE_DIR/Cargo.toml" $CARGO_PROFILE_FLAG || TEST_EXIT=$?

    log_info "==> Running .NET tests..."
    dotnet test "$DESKTOP_DIR/PrivStack.sln" -c "$DOTNET_CONFIG" --nologo -v minimal || TEST_EXIT=$?

    # Express integration tests (if config exists)
    WEB_ROOT="$(cd "$REPO_ROOT/.." && pwd)/PrivStack-Web"
    if [ -f "$WEB_ROOT/api/vitest.integration.config.js" ]; then
        log_info "==> Running Express integration tests..."
        (cd "$WEB_ROOT" && npx vitest run --config api/vitest.integration.config.js) || TEST_EXIT=$?
    fi

    # Teardown unless --persist
    if [ "$COMPOSE_UP" = true ]; then
        if [ "$PERSIST_TEST_DATA" = true ]; then
            log_info "==> --persist: leaving test containers running."
            log_debug "    MinIO console: http://localhost:9001  (privstack-test / privstack-test-secret)"
            log_debug "    MySQL:         localhost:3307          (root / test, db: privstack_test)"
            log_debug "    To tear down:  docker compose -f $COMPOSE_FILE down -v"
        else
            log_info "==> Tearing down test containers..."
            docker compose -f "$COMPOSE_FILE" down -v
        fi
    fi

    if [ "$TEST_EXIT" -ne 0 ]; then
        log_error "==> Tests failed (exit code $TEST_EXIT)."
        exit "$TEST_EXIT"
    fi
fi

# ── Step 4: Run ────────────────────────────────────────────────
if [ "$RUN_AFTER" = true ]; then
    # Map numeric log level to app env var
    case "$LOG_LEVEL" in
        0) APP_LOG_LEVEL="error" ;;
        1) APP_LOG_LEVEL="warn" ;;
        2) APP_LOG_LEVEL="info" ;;
        3) APP_LOG_LEVEL="debug" ;;
        *) APP_LOG_LEVEL="info" ;;
    esac

    if [ "$WITH_PLUGINS" = true ]; then
        # Isolated test instance — separate data directory from live
        mkdir -p "$TEST_DATA_DIR"
        log_info "==> Launching PrivStack Desktop (test mode — isolated data at $TEST_DATA_DIR)..."
        PRIVSTACK_DATA_DIR="$TEST_DATA_DIR" PRIVSTACK_LOG_LEVEL="$APP_LOG_LEVEL" \
            dotnet run --project "$DESKTOP_DIR/PrivStack.Desktop/PrivStack.Desktop.csproj" -c "$DOTNET_CONFIG" --no-build
    else
        log_info "==> Launching PrivStack Desktop..."
        PRIVSTACK_LOG_LEVEL="$APP_LOG_LEVEL" \
            dotnet run --project "$DESKTOP_DIR/PrivStack.Desktop/PrivStack.Desktop.csproj" -c "$DOTNET_CONFIG" --no-build
    fi
fi

log_info "==> Done."
