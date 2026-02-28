# PrivStack

A privacy-first, offline-capable personal information manager with encrypted local storage, peer-to-peer sync, and an extensible plugin architecture.

PrivStack keeps your data on your devices. There are no accounts, no cloud services, and no telemetry. Data is encrypted at rest with a master password, synced between your devices over direct peer-to-peer connections, and extended through a plugin system that lets third-party authors add new entity types and UI.

## Project Structure

```
core/                       Rust workspace — encryption, storage, CRDTs, sync engine, plugin host, FFI
desktop/
├── PrivStack.Services/     Shared core services, models, native FFI (no Avalonia)
├── PrivStack.Server/       Headless console binary — privstack-server (no Avalonia)
├── PrivStack.Desktop/      Avalonia GUI shell — window management, theming, plugin UI
├── PrivStack.Sdk/          Plugin SDK
├── PrivStack.UI.Adaptive/  Shared UI components
└── PrivStack.Desktop.Tests/
relay/                      Standalone Rust relay & DHT bootstrap node for NAT traversal
```

## Core (Rust)

The core is a Cargo workspace (edition 2024, Rust 1.85+) split into 15 crates. It owns all data logic and exposes a C FFI consumed by the desktop shell and future mobile clients.

### Encryption

Two-tier key architecture:

- **Master key** — derived from the user's password via Argon2id (OWASP 2023 parameters: 19 MiB memory, 2 iterations). Never stored; re-derived on every unlock.
- **Per-entity keys** — random 256-bit keys, each encrypted with the master key. This allows password changes without re-encrypting every entity, and makes it possible to share individual items by sharing their wrapped key.

All data at rest is encrypted with ChaCha20-Poly1305 (AEAD). Nonces are 96-bit random per encryption. Key material is zeroized on drop.

### Storage

DuckDB is the storage backend, chosen for its embedded OLAP capabilities (useful for analytics and future AI features). Three stores:

| Store | Purpose |
|---|---|
| **Entity store** | Entities with JSON payloads, schema-driven indexed columns, full-text search |
| **Event store** | Append-only log of all mutation events for sync replay |
| **Blob store** | Namespace-scoped binary objects with content-hash dedup |

Indexed fields are extracted from JSON via pointers declared in each entity schema, enabling queries without decrypting the full payload.

### Entity Model

Entities are schema-less JSON documents with a declared type. Plugins register schemas that define:

- Indexed fields and their types (text, number, tag, datetime, bool, relation, vector embedding, geo point, counter, etc.)
- A merge strategy for conflict resolution (last-writer-wins per document, last-writer-wins per field, or custom)
- Optional domain handlers for validation, post-load enrichment, and custom merge logic

### CRDTs

The CRDT crate provides five conflict-free replicated data types used throughout the sync engine:

| Type | Use |
|---|---|
| **Vector Clock** | Causal ordering of events across peers |
| **LWW Register** | Single-value properties (titles, settings) — highest timestamp wins, peer ID breaks ties |
| **PN Counter** | Distributed increment/decrement (budgets, counters) |
| **OR-Set** | Add-wins set membership (tags, collections) |
| **RGA** | Ordered sequences (text content, task lists) |

All types satisfy commutativity, associativity, and idempotency, guaranteeing convergence regardless of message order.

### Sync Engine

Sync is event-based and transport-agnostic. The engine supports two transports:

**P2P (libp2p)** — QUIC connections with Noise encryption. Discovery via mDNS on the local network and Kademlia DHT for wide-area. Devices pair using a 4-word sync code whose SHA-256 hash scopes the DHT namespace. Two-way approval is required before any data flows.

**Cloud storage** — Google Drive and iCloud as dumb file transports (encrypted blobs written to the user's own cloud storage).

The sync protocol exchanges vector clocks per entity, then transfers missing events in batches of up to 100. Events are applied using the entity's declared merge strategy. A subscription model provides real-time push for already-connected peers.

### Plugin Host

Plugins are WebAssembly components loaded via Wasmtime 33. The host enforces:

- Entity-type scoping — a plugin can only read and write its own declared entity types
- Memory limits (64 MB first-party, 32 MB third-party)
- CPU fuel budgets (~1B instructions first-party, 500M third-party)
- Call timeouts (5s / 3s)
- Permission-gated host imports (e.g., HTTP access requires explicit grant)

A policy engine loads allowlist/blocklist configuration and checks permissions before every host function call.

> **Planned:** A future version will introduce a custom template engine for plugin UI declarations, replacing the current JSON component tree approach. This will provide stronger sandboxing guarantees for third-party plugin UI rendering.

### FFI

The `privstack-ffi` crate exports a C ABI via a handle-based API. A single `PrivStackHandle` owns the runtime (Tokio), stores, vault, sync engine, and plugin host. Core operations are exposed as `extern "C"` functions; domain-specific calls route through a generic `privstack_execute(json) -> json` endpoint.

Consumers:
- .NET (P/Invoke) — desktop shell
- Swift (C interop) — iOS (planned)
- JNI — Android (planned)

### Vault

A generic encrypted blob store layered on top of the main DuckDB database. Each vault has its own password and salt. Multiple vaults can coexist (e.g., personal, work). Unlock verifies the password by decrypting a stored verification token. The derived key is held in memory while unlocked and zeroized on lock.

## Desktop Shell (Avalonia)

The desktop app is an Avalonia 11 / .NET 9 application that acts as a thin shell around the Rust core. It handles window management, navigation, theming, and plugin UI rendering — all data logic lives in Rust.

### Architecture

- **MVVM** with CommunityToolkit.Mvvm and Microsoft.Extensions.DependencyInjection
- **Plugin registry** discovers, initializes, and manages the lifecycle of plugins (discovered → initializing → initialized → active → deactivated)
- **Capability broker** enables cross-plugin communication — plugins declare capabilities (timers, reminders, linkable items, deep links, search) and other plugins discover providers at runtime
- **SDK host** routes all data operations from plugins through FFI to Rust, with a reader-writer lock to prevent calls during workspace switches

### Plugin UI Rendering

Native (.NET) plugins use standard Avalonia XAML views. Wasm plugins return a JSON component tree that the **adaptive view renderer** translates into live Avalonia controls.

Supported components include layout containers (stack, grid, split pane, scroll), data display (text, badge, icon, image, list), input controls (button, text input, toggle, dropdown), and rich components (block editor, HTML content, graph view).

A built-in template engine evaluates `$for` loops, `$if` conditionals, and `{{ expression | filter }}` interpolation within the JSON tree before rendering.

### Theming and Responsive Layout

Seven themes ship by default (Dark, Light, Sage, Lavender, Azure, Slate, Ember). All visual properties use dynamic resources, so theme switches propagate instantly.

The responsive layout service adapts to three breakpoints based on content area width:

| Mode | Width | Behavior |
|---|---|---|
| Compact | < 700px | Sidebar collapsed, info panel hidden |
| Normal | 700–1100px | Balanced layout |
| Wide | > 1100px | Full sidebar, content, and info panel |

Font scaling is a separate accessibility multiplier applied on top of theme sizes.

### Shell Features

- **Command palette** (Cmd/Ctrl+K) — searches across plugin commands, navigation items, deep links, and entity content
- **Info panel** — right-side collapsible panel showing backlinks, local knowledge graph, and entity metadata
- **Setup wizard** — first-run flow for data directory, master password, and theme selection
- **Workspace switching** — multiple data directories with full plugin re-initialization on switch
- **Backup service** — scheduled automatic backups with configurable frequency and retention
- **Sensitive lock** — secondary timeout-based lock for high-value features (passwords, vault access)
- **Auto-update** — checks for and installs updates

## Headless Server (`privstack-server`)

A standalone console binary that runs PrivStack as an API-only server with no GUI or Avalonia dependency. Designed for Raspberry Pi, VPS, container, and CI/CD deployments.

### Architecture

```
PrivStack.Services/     Shared core — services, models, native FFI (no Avalonia)
PrivStack.Server/       Headless binary — console UI, setup wizard, TLS, policy
PrivStack.Desktop/      GUI shell — Avalonia UI, delegates to Services
PrivStack.Sdk/          Plugin SDK
```

`PrivStack.Services` is the shared library consumed by both Desktop and Server. Server has zero Avalonia dependencies.

### First-Run Setup

```bash
# Interactive setup wizard — creates workspace, sets master password,
# configures network, TLS, and unlock method
./privstack-server --setup
```

The wizard walks through:

1. **Workspace** — name for the data workspace
2. **Master password** — encrypts all data (8+ characters, confirmed)
3. **Unlock method** — how the server authenticates on startup:
   - Password every start (most secure)
   - OS keyring (macOS Keychain / Windows Credential Manager / Linux `secret-tool`)
   - Environment variable (`PRIVSTACK_MASTER_PASSWORD`)
4. **Network** — bind address and port (default: `127.0.0.1:9720`)
5. **TLS** — optional HTTPS with two modes:
   - Manual certificate (PFX/P12 or PEM + private key)
   - Let's Encrypt (automatic free certificate via ACME)
6. **Recovery phrase** — 12-word mnemonic for data recovery (write it down!)

Setup generates an API key and saves configuration to `headless-config.json`.

### Running

```bash
# Start server (prompts for password or uses keyring/env var)
./privstack-server

# With env var authentication
PRIVSTACK_MASTER_PASSWORD=<pw> ./privstack-server

# Select specific workspace
./privstack-server --workspace "Work"

# Override network settings
./privstack-server --port 8080 --bind 0.0.0.0
```

### Command-Line Flags

| Flag | Description |
|---|---|
| `--setup` | Run the interactive setup wizard (first-run or re-run) |
| `--setup-network` | Re-configure bind address and port only |
| `--setup-tls` | Re-configure TLS settings only |
| `--setup-policy` | Re-configure enterprise policy only |
| `--workspace <name\|id>` | Target workspace (default: active workspace) |
| `--port <N>` | Override API port (default: from config or 9720) |
| `--bind <addr>` | Override bind address (default: from config or `127.0.0.1`) |
| `--show-api-key` | Print the current API key and exit |
| `--generate-api-key` | Generate a new API key, save it, print it, and exit |

### Authentication

The server acquires the master password in priority order:

1. `PRIVSTACK_MASTER_PASSWORD` environment variable (cleared after reading)
2. OS keyring (if unlock method is `OsKeyring`)
3. Interactive stdin prompt (if terminal is attached)
4. Stdin line read (if stdin is redirected, e.g., `echo pw | privstack-server`)

API endpoints require an `X-API-Key` header (or `Authorization: Bearer <key>`). The unauthenticated `GET /api/v1/status` endpoint returns server health and workspace info.

### TLS / HTTPS

Two modes, configured via `--setup-tls` or the full setup wizard:

**Manual certificate:**

```bash
./privstack-server --setup-tls
# Choose "Manual certificate"
# Provide path to .pfx/.p12 file (with optional password)
# Or provide .pem certificate + .pem private key
```

**Let's Encrypt (automatic):**

```bash
./privstack-server --setup-tls
# Choose "Let's Encrypt"
# Provide domain name, email, accept ToS
# Server listens on HTTPS (configured port) + HTTP port 80 for ACME challenges
```

Requirements for Let's Encrypt:
- Server must be publicly accessible on the specified domain
- Port 80 must be open for ACME HTTP-01 domain validation
- A valid domain name (not an IP address)

Certificates are automatically renewed before expiration. A staging mode is available for testing (not trusted by browsers).

### Enterprise Policy

An optional TOML policy file enables centralized control over server behavior. Configured via `--setup-policy` or by setting `policy_path` in `headless-config.json`.

```toml
# ~/.privstack/policy.toml

[authority]
public_key = "base64-ecdsa-p256-public-key"
signature  = "base64-ecdsa-p256-signature"

[plugins]
mode = "allowlist"                          # "allowlist", "blocklist", or "disabled"
list = ["tasks", "notes", "calendar"]

[network]
allowed_cidrs = ["192.168.1.0/24", "10.0.0.0/8"]

[api]
require_tls = true                          # refuse to start without TLS

[audit]
enabled  = true
log_path = "/var/log/privstack/audit.log"   # JSON Lines format
level    = "all"                            # "all", "write" (skip GETs), or "auth"
```

**Enforcement points:**

| Section | Effect |
|---|---|
| `[plugins]` | Restricts which plugins can load (allowlist or blocklist) |
| `[network]` | Blocks API requests from IPs outside allowed CIDR ranges |
| `[api]` | Requires TLS — server refuses to start if HTTPS is not configured |
| `[audit]` | Logs API requests to a JSON Lines file with configurable verbosity |

**Signature verification:** The optional `[authority]` section uses ECDSA P-256 to sign the policy body (all sections except `[authority]`). If present, the server verifies the signature on load and refuses to start if it has been tampered with.

### Configuration Files

| File | Location | Purpose |
|---|---|---|
| `headless-config.json` | `~/.privstack/` | Server-specific: unlock method, network, TLS, policy path |
| `settings.json` | `~/.privstack/` | Shared: API key, API port, setup complete marker |
| `policy.toml` | Admin-defined path | Enterprise policy (optional) |

### Environment Variables

| Variable | Description |
|---|---|
| `PRIVSTACK_MASTER_PASSWORD` | Master password (avoids interactive prompt) |
| `PRIVSTACK_DATA_DIR` | Override data directory (default: `~/.privstack/`) |
| `PRIVSTACK_LOG_LEVEL` | Log level: `Verbose`, `Debug`, `Information`, `Warning`, `Error`, `Fatal` |

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Clean shutdown |
| `1` | Configuration error (missing setup, invalid policy, etc.) |
| `2` | Authentication failure |
| `3` | Port already in use |
| `4` | Database locked (another instance running) |

### API Usage

```bash
# Check server status (no auth required)
curl http://127.0.0.1:9720/api/v1/status

# List available routes
curl -H "X-API-Key: <key>" http://127.0.0.1:9720/api/v1/routes

# Example: list tasks
curl -H "X-API-Key: <key>" http://127.0.0.1:9720/api/v1/tasks

# With HTTPS
curl --cacert cert.pem -H "X-API-Key: <key>" https://privstack.example.com:9720/api/v1/status
```

### Building

```bash
cd desktop/PrivStack.Server
dotnet build

# The output binary is privstack-server (or privstack-server.exe on Windows)
```

### Platforms

| Platform | Transport | Native Library |
|---|---|---|
| macOS (arm64, x64) | libprivstack_ffi.dylib | QUIC P2P |
| Windows (x64) | privstack_ffi.dll | QUIC P2P + WebView |
| Linux (x64) | libprivstack_ffi.so | QUIC P2P |

WebView is currently disabled on macOS due to .NET 9 MacCatalyst compatibility issues.

## Relay

A lightweight, stateless Rust server that helps PrivStack clients find each other and connect through NATs.

- **Kademlia DHT** — clients publish their presence under a namespace derived from their sync code hash; the relay bootstraps the routing table
- **libp2p relay protocol** — forwards traffic between peers that cannot establish direct connections
- **HTTP identity API** (`GET /api/v1/identity`) — returns the relay's peer ID and addresses so clients don't need to hardcode them

The relay listens on UDP 4001 (QUIC) and TCP 4002 (HTTP). It stores no user data. Deployment is a single binary managed by systemd.

See [Relay README](relay/README.md) for deployment instructions.

## Building

### Core

```bash
cd core
cargo build --release
```

### Desktop (GUI)

```bash
cd desktop/PrivStack.Desktop
dotnet build
```

### Server (Headless)

```bash
cd desktop/PrivStack.Server
dotnet build
# Output: desktop/PrivStack.Server/bin/Debug/net9.0/privstack-server
```

### Relay

```bash
cd relay
cargo build --release
```

## Documentation

Detailed architecture documentation is available in the [wiki](wiki/):

- [Architecture Overview](wiki/architecture.md)
- [Entity Model](wiki/entity-model.md)
- [CRDTs](wiki/crdts.md)
- [Sync Engine](wiki/sync-engine.md)
- [Cryptography](wiki/cryptography.md)
- [Storage](wiki/storage.md)
- [FFI Layer](wiki/ffi.md)
- [Desktop SDKs](wiki/sdks.md)
- [Relay Server](wiki/relay.md)

## License

[PolyForm Internal Use License 1.0.0](LICENSE)
