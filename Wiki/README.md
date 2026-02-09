# PrivStack-IO Wiki

Developer documentation for the PrivStack-IO engine.

## Contents

### Architecture
- [Architecture Overview](architecture-overview.md) — System layers, data flow, and design principles
- [Entity Model](entity-model.md) — Entities, schemas, field types, and merge strategies

### Core Systems
- [CRDT System](crdt-system.md) — Conflict-free replicated data types used for sync
- [Sync Engine](sync-engine.md) — P2P and cloud sync protocol, pairing, and policies
- [Cryptography](cryptography.md) — Encryption, key derivation, and vault architecture
- [Storage Layer](storage-layer.md) — DuckDB persistence, entity store, and event store

### Plugin System
- [Plugin Architecture](plugin-architecture.md) — Plugin lifecycle, SDK, capabilities, and Wasm sandbox
- [Plugin Package Format](plugin-package-format.md) — `.ppk` format, signing, and distribution

### Operations
- [Relay Deployment](relay-deployment.md) — Deploying the P2P relay/bootstrap node
- [FFI Reference](ffi-reference.md) — C ABI boundary for .NET, Android, and iOS integration
