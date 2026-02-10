# FFI Reference

The `privstack-ffi` crate exposes the Rust core as a C-compatible shared library consumed by:

- **.NET/Avalonia** — via P/Invoke
- **Android** — via JNI
- **iOS** — via Swift C interop

## Design Principles

- **Zero domain logic** — FFI is a thin translation layer; all logic lives in the core crates
- **C-compatible types only** — `*const c_char`, `c_int`, opaque pointers
- **Error codes** — All functions return `PrivStackError` enum values
- **Tokio runtime** — Async operations run on a managed Tokio runtime initialized once

## Native Library Names

| Platform | Library |
|----------|---------|
| macOS | `libprivstack_ffi.dylib` |
| Linux | `libprivstack_ffi.so` |
| Windows | `privstack_ffi.dll` |

## Error Codes

```rust
#[repr(C)]
pub enum PrivStackError {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    JsonError = 3,
    StorageError = 4,
    NotFound = 5,
    NotInitialized = 6,
    SyncNotRunning = 7,
    SyncAlreadyRunning = 8,
    SyncError = 9,
    PeerNotFound = 10,
    AuthError = 11,
    CloudError = 12,
    LicenseInvalidFormat = 13,
    LicenseInvalidChecksum = 14,
    LicenseExpired = 15,
    LicenseNotActivated = 16,
    LicenseActivationFailed = 17,
    InvalidSyncCode = 18,
    PeerNotTrusted = 19,
    PairingError = 20,
    VaultLocked = 21,
    VaultNotFound = 22,
    PluginError = 23,
    PluginNotFound = 24,
    PluginPermissionDenied = 25,
    Unknown = 99,
}
```

## Exposed Subsystems

The FFI layer provides C ABI functions for:

| Subsystem | Operations |
|-----------|-----------|
| **Runtime** | Initialize/shutdown Tokio runtime |
| **Storage** | Entity CRUD, query, search, link management |
| **Events** | Append events, query event log |
| **Vault** | Initialize, unlock, lock, blob read/write/delete, password change |
| **Blob Store** | Namespace-scoped unencrypted blob CRUD |
| **Sync** | Start/stop sync, force sync, peer management |
| **Pairing** | Generate sync codes, verify codes, trust/revoke peers |
| **Cloud** | Configure Google Drive/iCloud, trigger cloud sync |
| **License** | Validate keys, activate/deactivate, device fingerprint |
| **Plugin Host** | Load Wasm plugins, invoke plugin functions |

## .NET P/Invoke Pattern

The desktop app loads the native library based on the current OS:

```xml
<!-- macOS -->
<None Include="$(RustLibraryPath)\libprivstack_ffi.dylib"
      CopyToOutputDirectory="PreserveNewest" />

<!-- Windows -->
<None Include="$(RustLibraryPath)\privstack_ffi.dll"
      CopyToOutputDirectory="PreserveNewest" />

<!-- Linux -->
<None Include="$(RustLibraryPath)\libprivstack_ffi.so"
      CopyToOutputDirectory="PreserveNewest" />
```

C# services use `[DllImport]` or `LibraryImport` to call into the native library. String parameters are marshalled as `c_char*` (null-terminated UTF-8). JSON payloads are serialized to strings, passed through FFI, and deserialized on the Rust side.

## Cross-Compilation

For building the native library for a different platform, place the compiled library in the `dist/` folder:

```
dist/
  macos-arm64/native/libprivstack_ffi.dylib
  macos-x64/native/libprivstack_ffi.dylib
  windows-x64/native/privstack_ffi.dll
  linux-x64/native/libprivstack_ffi.so
```

The `.csproj` conditionally includes the correct library based on `RuntimeIdentifier`.

## Thread Safety

- The Tokio runtime is initialized once and shared across all FFI calls
- `Arc<Mutex<...>>` and `Arc<TokioMutex<...>>` protect shared state
- Sync operations that block are dispatched to the Tokio runtime via `block_on`
- The FFI layer is safe to call from any thread
