# PrivStack Plugin SDK Reference

> The single document a plugin developer needs to build, test, and ship a Wasm plugin for PrivStack.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Quick Start](#2-quick-start)
3. [Project Structure](#3-project-structure)
4. [Plugin Trait](#4-plugin-trait)
5. [Optional Capabilities](#5-optional-capabilities)
6. [Export Macro](#6-export-macro)
7. [Host API Reference](#7-host-api-reference)
8. [SDK Types Reference](#8-sdk-types-reference)
9. [Sidecar Files](#9-sidecar-files)
10. [Packaging (.ppk)](#10-packaging-ppk)
11. [Permissions & Sandbox](#11-permissions--sandbox)
12. [Complete Example: RSS Reader](#12-complete-example-rss-reader)

---

## 1. Overview

PrivStack plugins run inside a **WebAssembly (Wasm) sandbox** using the [Component Model](https://component-model.bytecodealliance.org/). The host and guest communicate through a WIT (Wasm Interface Types) contract — the plugin cannot access the filesystem, network, or any OS resource directly. Every capability is mediated by host-provided imports.

### Data Flow

```
┌─────────────────────────────────────────────────────────┐
│  Host (PrivStack App)                                   │
│                                                         │
│  ┌──────────┐   WIT imports    ┌──────────────────┐     │
│  │ Plugin   │ ◄──────────────► │  Wasm Sandbox    │     │
│  │ Host     │   WIT exports    │  ┌────────────┐  │     │
│  │          │ ◄──────────────  │  │ Your       │  │     │
│  │ • SDK    │                  │  │ Plugin     │  │     │
│  │ • Store  │  sdk::send()     │  │ (Rust)     │  │     │
│  │ • Vault  │ ────────────►    │  └────────────┘  │     │
│  │ • Net    │                  └──────────────────┘     │
│  └──────────┘                                           │
└─────────────────────────────────────────────────────────┘
```

### Plugin Lifecycle

```
Default::default()
    │
    ▼
initialize()  ──► return false → plugin disabled
    │ true
    ▼
activate()  ←──── load state from entity store
    │
    ▼
┌──────────────── active loop ────────────────┐
│  on_navigated_to()                          │
│  handle_command(name, args) → JSON response │
│  get_view_state() / get_view_data()         │
│  on_navigated_from()                        │
└─────────────────────────────────────────────┘
    │
    ▼
deactivate()  ──── clear transient state
    │
    ▼
dispose()     ──── final cleanup
```

### Two Worlds

| World | Use Case | Extra Import |
|-------|----------|--------------|
| `plugin-world` | Standard plugins | — |
| `agent-plugin-world` | Agent/analytics plugins | `agent` (cross-entity queries, analytics SQL, cross-plugin commands) |

---

## 2. Quick Start

### Prerequisites

- Rust toolchain with `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- The `privstack-plugin-sdk` crate (workspace dependency)

### Scaffold a Minimal Plugin

**1. `Cargo.toml`**

```toml
[package]
name = "privstack-plugin-hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
privstack-plugin-sdk = { path = "../../core/privstack-plugin-sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = { version = "0.38", default-features = false, features = ["macros"] }
```

**2. `metadata.json`**

```json
{
  "id": "community.hello",
  "name": "Hello",
  "description": "A minimal example plugin",
  "version": "0.1.0",
  "author": "You",
  "icon": "Smile",
  "navigation_order": 1000,
  "category": "extension",
  "can_disable": true,
  "is_experimental": true
}
```

**3. `schemas.json`**

```json
[
  {
    "entity_type": "greeting",
    "indexed_fields": [
      {
        "field_path": "/message",
        "field_type": "text",
        "searchable": true,
        "vector_dim": null,
        "enum_options": null
      }
    ],
    "merge_strategy": "lww-per-field"
  }
]
```

**4. `template.json`**

```json
{
  "type": "stack",
  "spacing": 8,
  "children": [
    { "type": "text", "value": "Hello from a Wasm plugin!", "style": "heading" }
  ]
}
```

**5. `src/lib.rs`**

```rust
use privstack_plugin_sdk::prelude::*;

// Generate all WIT export glue — no optional capabilities.
privstack_plugin_sdk::privstack_wasm_export!(HelloPlugin);

pub struct HelloPlugin;

impl Default for HelloPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for HelloPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "community.hello".into(),
            name: "Hello".into(),
            description: "A minimal example plugin".into(),
            version: "0.1.0".into(),
            author: "You".into(),
            icon: Some("Smile".into()),
            navigation_order: 1000,
            category: PluginCategory::Extension,
            can_disable: true,
            is_experimental: true,
        }
    }

    fn entity_schemas(&self) -> Vec<EntitySchema> {
        vec![EntitySchema {
            entity_type: "greeting".into(),
            indexed_fields: vec![
                IndexedField::text("/message", true),
            ],
            merge_strategy: MergeStrategy::LwwPerField,
        }]
    }

    fn initialize(&mut self) -> bool {
        true
    }
}
```

**6. Build**

```bash
cargo build --target wasm32-wasip2 --release
```

The output `.wasm` file is at `target/wasm32-wasip2/release/privstack_plugin_hello.wasm`.

---

## 3. Project Structure

```
privstack-plugin-hello/
├── Cargo.toml              # Crate config, must set crate-type = ["cdylib", "rlib"]
├── metadata.json           # Plugin identity (loaded by host at install time)
├── schemas.json            # Entity type declarations with indexed fields
├── template.json           # FluidUI declarative template (optional)
├── command_palettes.json   # Command palette definitions (optional)
└── src/
    └── lib.rs              # Plugin impl + privstack_wasm_export!() call
```

| File | Required | Purpose |
|------|----------|---------|
| `Cargo.toml` | Yes | Rust crate manifest. `crate-type = ["cdylib", "rlib"]` is required for Wasm output. |
| `metadata.json` | Yes | Plugin identity loaded by the host at install time. Must match `Plugin::metadata()`. |
| `schemas.json` | Yes | Entity schemas declaring what data types the plugin manages. Must match `Plugin::entity_schemas()`. |
| `template.json` | No | FluidUI declarative template for rendering. If present, the host evaluates it with data from `TemplateDataProvider::get_view_data()`. |
| `command_palettes.json` | No | Custom command palettes (e.g., slash-command menus). |
| `src/lib.rs` | Yes | Plugin trait implementation and `privstack_wasm_export!()` macro call. |

---

## 4. Plugin Trait

Every plugin must implement the `Plugin` trait. Three methods are required; the rest have defaults.

```rust
pub trait Plugin {
    // ── Required ──────────────────────────────────────────
    fn metadata(&self) -> PluginMetadata;
    fn entity_schemas(&self) -> Vec<EntitySchema>;
    fn initialize(&mut self) -> bool;

    // ── Optional (have defaults) ──────────────────────────
    fn navigation_item(&self) -> Option<NavigationItem> { None }
    fn commands(&self) -> Vec<CommandDefinition> { Vec::new() }
    fn activate(&mut self) {}
    fn deactivate(&mut self) {}
    fn on_navigated_to(&mut self) {}
    fn on_navigated_from(&mut self) {}
    fn dispose(&mut self) {}
    fn get_view_state(&self) -> String { "{}".to_string() }
    fn handle_command(&mut self, _name: &str, _args: &str) -> String { "{}".to_string() }
}
```

### Method Details

| Method | When Called | Notes |
|--------|-----------|-------|
| `metadata()` | Host queries identity | Return static plugin info. Must agree with `metadata.json`. |
| `entity_schemas()` | Host registers entity types | Must agree with `schemas.json`. |
| `initialize()` | Once after instantiation | Return `false` to abort plugin load. |
| `navigation_item()` | Host builds sidebar | Return `Some(...)` to appear in navigation. |
| `commands()` | Host registers command palette entries | Each `CommandDefinition` appears in global search. |
| `activate()` | Plugin becomes the active view | Load state from entity store here. |
| `deactivate()` | Plugin leaves the active view | Clear transient state to free memory. |
| `on_navigated_to()` | User navigates to this plugin | Trigger refresh/reload if needed. |
| `on_navigated_from()` | User navigates away | Pause background work. |
| `dispose()` | Plugin unloaded | Final cleanup. |
| `get_view_state()` | Host requests UI state | Return JSON for ViewModelProxy rendering. Legacy path — prefer `TemplateDataProvider`. |
| `handle_command(name, args)` | UI or host sends a command | `args` is JSON. Return JSON response. |

---

## 5. Optional Capabilities

Implement these traits to opt into additional host integration. Each must be listed in the `privstack_wasm_export!()` macro to generate real WIT exports.

### LinkableItemProvider

Allows other plugins to search and link to items from your plugin.

```rust
pub trait LinkableItemProvider {
    fn link_type(&self) -> &str;
    fn link_type_display_name(&self) -> &str;
    fn link_type_icon(&self) -> &str;
    fn search_items(&self, query: &str, max_results: u32) -> Vec<LinkableItem>;
    fn get_item_by_id(&self, id: &str) -> Option<LinkableItem>;
}
```

### DeepLinkTarget

Allows the host to navigate directly to a specific item.

```rust
pub trait DeepLinkTarget {
    fn link_type(&self) -> &str;
    fn navigate_to_item(&mut self, item_id: &str);
}
```

### TimerBehavior

Adds timer/stopwatch functionality.

```rust
pub trait TimerBehavior {
    fn start_timer(&mut self, item_id: &str);
    fn pause_timer(&mut self);
    fn resume_timer(&mut self);
    fn stop_timer(&mut self) -> TimerResult;
    fn get_timer_state(&self) -> TimerState;
}
```

### ShutdownAware

Called when the host application is shutting down — use for graceful cleanup.

```rust
pub trait ShutdownAware {
    fn on_shutdown(&mut self);
}
```

### TemplateDataProvider

For plugins that ship a `template.json` sidecar. Returns raw data JSON that the host evaluates against the template.

```rust
pub trait TemplateDataProvider {
    fn get_view_data(&self) -> String { "{}".to_string() }
}
```

---

## 6. Export Macro

The `privstack_wasm_export!()` macro generates all WIT binding boilerplate. It creates:

- The `wit_gen` module (via `wit_bindgen::generate!`)
- A `Mutex<Option<YourPlugin>>` singleton
- WIT `Guest` trait implementations that delegate to your `Plugin` (and capability) impls
- Stub implementations for unlisted capabilities

### No Capabilities

```rust
privstack_plugin_sdk::privstack_wasm_export!(MyPlugin);
```

### With Capabilities

```rust
privstack_plugin_sdk::privstack_wasm_export!(MyPlugin, [LinkableItemProvider, TemplateDataProvider]);
```

### Available Capability Flags

| Flag | Trait |
|------|-------|
| `LinkableItemProvider` | `LinkableItemProvider` |
| `DeepLinkTarget` | `DeepLinkTarget` |
| `TimerBehavior` | `TimerBehavior` |
| `ShutdownAware` | `ShutdownAware` |
| `TemplateDataProvider` | `TemplateDataProvider` |

### Requirements on Your Plugin Type

- Must implement `Default` (the macro calls `<YourPlugin>::default()` to create the singleton)
- Must implement `Plugin`
- Must implement each listed capability trait

---

## 7. Host API Reference

These are the WIT imports your plugin can call at runtime. They are accessed through the generated `wit_gen` module.

### 7.1 `sdk` — Entity CRUD

**Tier 1** (always granted). CRUD is scoped to the plugin's declared entity types only.

```rust
// WIT signature
sdk::send(message: &SdkMessage) -> SdkResponse;
sdk::search(query: &str, entity_types: Option<&[String]>, limit: u32) -> SdkResponse;
```

Usage pattern:

```rust
use crate::wit_gen::privstack::plugin::sdk;
use crate::wit_gen::privstack::plugin::types as wit_types;

let msg = wit_types::SdkMessage {
    action: wit_types::SdkAction::Create,
    entity_type: "note".to_string(),
    entity_id: Some("note-001".to_string()),
    payload: Some(r#"{"title":"Hello"}"#.to_string()),
    parameters: vec![],
    source: None,
};
let resp = sdk::send(&msg);
if resp.success {
    // resp.data contains the created entity JSON
}
```

#### SdkAction Variants

| Action | Description |
|--------|-------------|
| `Create` | Create a new entity |
| `Read` | Read a single entity by ID |
| `Update` | Upsert an entity |
| `Delete` | Permanently delete |
| `ListAll` | List all entities of a type |
| `Query` | Query with parameters |
| `Trash` | Soft-delete |
| `Restore` | Restore from trash |
| `Link` | Create cross-entity link |
| `Unlink` | Remove a link |
| `GetLinks` | List links for an entity |
| `SemanticSearch` | Vector similarity search |

### 7.2 `settings` — Key-Value Preferences

**Tier 1** (always granted). Plugin-scoped key-value store.

```rust
settings::get(key: &str, default_value: &str) -> String;
settings::set(key: &str, value: &str);
settings::remove(key: &str);
```

### 7.3 `logger` — Structured Logging

**Tier 1** (always granted). Logs are tagged with the plugin ID automatically.

```rust
logger::debug(message: &str);
logger::info(message: &str);
logger::warn(message: &str);
logger::error(message: &str);
```

### 7.4 `navigation` — Plugin-to-Plugin Navigation

**Tier 1** (always granted).

```rust
navigation::navigate_to(plugin_id: &str);
navigation::navigate_back();
```

### 7.5 `vault` — Encrypted Blob Storage

**Tier 2** (JIT prompted — user sees a permission dialog on first use).

```rust
vault::is_initialized(vault_id: &str) -> bool;
vault::initialize(vault_id: &str, password: &str);
vault::unlock(vault_id: &str, password: &str);
vault::lock(vault_id: &str);
vault::blob_store(vault_id: &str, blob_id: &str, data: &[u8]);
vault::blob_read(vault_id: &str, blob_id: &str) -> Vec<u8>;
vault::blob_delete(vault_id: &str, blob_id: &str);
```

### 7.6 `linking` — Cross-Plugin Item Linking

**Tier 2** (JIT prompted).

```rust
linking::search_items(query: &str, max_results: u32) -> Vec<LinkableItem>;
linking::get_item_by_id(item_id: &str) -> Option<LinkableItem>;
linking::get_all_providers() -> Vec<LinkProviderInfo>;
linking::query_all(query: &str, max_results: u32) -> Vec<LinkableItem>;
```

### 7.7 `dialogs` — Confirmation & File Dialogs

**Tier 2** (JIT prompted).

```rust
dialogs::show_confirmation(title: &str, message: &str) -> bool;
dialogs::show_open_file(title: &str, filters: &[String]) -> Option<String>;
```

### 7.8 `state-notify` — Push UI Updates

**Tier 1** (always granted). Call this after mutating state to tell the host to re-render.

```rust
state_notify::notify_state_changed(json_patch: Option<&str>);
```

Pass `None` for a full re-fetch, or a JSON Patch string for incremental updates.

### 7.9 `network` — HTTP Fetch (Sandboxed)

**Tier 3** (install-time permission required). The host performs the actual HTTP request.

```rust
network::fetch_url(
    url: &str,
    method: &str,
    headers: &[HttpHeader],
    body: Option<&[u8]>,
) -> Result<HttpResponse, String>;
```

```rust
// HttpHeader
record HttpHeader { name: String, value: String }

// HttpResponse
record HttpResponse { status: u16, headers: Vec<HttpHeader>, body: Vec<u8> }
```

### 7.10 `agent` — Cross-Entity Queries & Analytics (Extended World)

Only available in `agent-plugin-world`. Requires `cross-entity-read` or `cross-plugin-command` permissions.

```rust
agent::query_entities(entity_type: &str, query: &str, limit: u32) -> SdkResponse;
agent::run_analytics(sql: &str, params: &[String]) -> SdkResponse;
agent::send_command(target_plugin_id: &str, command: &str, args: &str) -> SdkResponse;
```

- `run_analytics` only allows `SELECT` statements against permitted entity types.
- `send_command` dispatches to another plugin's `handle_command`.

---

## 8. SDK Types Reference

All types are in `privstack_plugin_sdk::types` (re-exported via `prelude::*`).

### PluginMetadata

```rust
pub struct PluginMetadata {
    pub id: String,                 // "privstack.notes" or "community.my-plugin"
    pub name: String,               // Display name
    pub description: String,
    pub version: String,            // SemVer
    pub author: String,
    pub icon: Option<String>,       // Lucide icon name
    pub navigation_order: u32,      // Sidebar position
    pub category: PluginCategory,
    pub can_disable: bool,
    pub is_experimental: bool,
}
```

`Default` provides: `version = "0.1.0"`, `navigation_order = 1000`, `category = Extension`, `can_disable = true`.

### PluginCategory

```rust
pub enum PluginCategory {
    Productivity,   // Core productivity tools
    Security,       // Vault, passwords, 2FA
    Communication,  // Chat, email
    Information,    // RSS, weather, reference
    Utility,        // Timers, calculators
    Extension,      // Third-party default
}
```

Serializes as lowercase (`"productivity"`, `"utility"`, etc.).

### NavigationItem

```rust
pub struct NavigationItem {
    pub id: String,
    pub display_name: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,          // Lucide icon name
    pub tooltip: Option<String>,
    pub order: u32,                    // Sidebar position
    pub show_badge: bool,
    pub badge_count: u32,
    pub shortcut_hint: Option<String>, // e.g. "Cmd+1"
}
```

### EntitySchema

```rust
pub struct EntitySchema {
    pub entity_type: String,
    pub indexed_fields: Vec<IndexedField>,
    pub merge_strategy: MergeStrategy,
}
```

### IndexedField

```rust
pub struct IndexedField {
    pub field_path: String,            // JSON pointer, e.g. "/title"
    pub field_type: FieldType,
    pub searchable: bool,
    pub vector_dim: Option<u16>,       // Only for FieldType::Vector
    pub enum_options: Option<Vec<String>>, // Only for FieldType::Enumeration
}
```

#### Ergonomic Constructors

| Constructor | Field Type | Searchable | Extra |
|-------------|-----------|------------|-------|
| `IndexedField::text(path, searchable)` | `Text` | param | — |
| `IndexedField::tag(path)` | `Tag` | always `true` | — |
| `IndexedField::datetime(path)` | `DateTime` | `false` | — |
| `IndexedField::number(path)` | `Number` | `false` | — |
| `IndexedField::boolean(path)` | `Boolean` | `false` | — |
| `IndexedField::vector(path, dim)` | `Vector` | `false` | `vector_dim = Some(dim)` |
| `IndexedField::counter(path)` | `Counter` | `false` | — |
| `IndexedField::relation(path)` | `Relation` | `false` | — |
| `IndexedField::decimal(path)` | `Decimal` | `false` | — |
| `IndexedField::json(path)` | `Json` | `false` | — |
| `IndexedField::enumeration(path, options)` | `Enumeration` | `false` | `enum_options = Some(options)` |
| `IndexedField::geo_point(path)` | `GeoPoint` | `false` | — |
| `IndexedField::duration(path)` | `Duration` | `false` | — |

### FieldType

```rust
pub enum FieldType {
    Text,        // UTF-8 string
    Tag,         // Array of strings, always searchable
    DateTime,    // Unix epoch or ISO 8601
    Number,      // i64
    Boolean,
    Vector,      // Float array for embeddings (set vector_dim)
    Counter,     // CRDT counter
    Relation,    // Entity ID foreign key
    Decimal,     // Fixed-point decimal
    Json,        // Arbitrary JSON blob
    Enumeration, // Fixed set of string options (set enum_options)
    GeoPoint,    // Lat/lon pair
    Duration,    // Time duration in ms
}
```

### MergeStrategy

```rust
pub enum MergeStrategy {
    LwwDocument,  // Last-Writer-Wins at document level
    LwwPerField,  // Last-Writer-Wins per field (recommended)
    Custom,       // Plugin handles merge logic
}
```

Serializes as kebab-case: `"lww-document"`, `"lww-per-field"`, `"custom"`.

### SdkMessage

```rust
pub struct SdkMessage {
    pub action: SdkAction,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub payload: Option<String>,           // JSON body
    pub parameters: Vec<(String, String)>, // Query params
    pub source: Option<String>,            // Caller plugin ID
}
```

### SdkAction

```rust
pub enum SdkAction {
    Create, Read, Update, Delete,
    List, Query, Trash, Restore,
    Link, Unlink, GetLinks, SemanticSearch,
}
```

Serializes as kebab-case: `"create"`, `"list"`, `"get-links"`, `"semantic-search"`.

### SdkResponse

```rust
pub struct SdkResponse {
    pub success: bool,
    pub error_code: Option<u32>,
    pub error_message: Option<String>,
    pub data: Option<String>,              // JSON payload
}

impl SdkResponse {
    pub fn is_ok(&self) -> bool;
    pub fn parse_data<T: DeserializeOwned>(&self) -> Option<T>;
}
```

### LinkableItem

```rust
pub struct LinkableItem {
    pub id: String,
    pub link_type: String,       // e.g. "note", "rss_article"
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub modified_at: u64,        // Unix epoch ms
}
```

### CommandDefinition

```rust
pub struct CommandDefinition {
    pub name: String,            // Displayed in command palette
    pub description: String,
    pub keywords: String,        // Space-separated search terms
    pub category: String,        // Grouping label
    pub icon: Option<String>,    // Lucide icon name
}
```

### TimerState

```rust
pub struct TimerState {
    pub is_active: bool,
    pub is_running: bool,
    pub elapsed_ms: u64,
    pub item_title: Option<String>,
}
```

### TimerResult

```rust
pub struct TimerResult {
    pub item_id: String,
    pub elapsed_ms: u64,
}
```

---

## 9. Sidecar Files

Sidecar JSON files are loaded by the host at plugin install/activation time. They live alongside your `Cargo.toml`.

### 9.1 `metadata.json`

Plugin identity. Must agree with `Plugin::metadata()` return value.

```json
{
  "id": "privstack.rss",
  "name": "Pulse",
  "description": "RSS/Atom feed reader with reader mode",
  "version": "1.0.0",
  "author": "PrivStack",
  "icon": "Rss",
  "navigation_order": 350,
  "category": "utility",
  "can_disable": true,
  "is_experimental": false
}
```

#### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique reverse-domain ID (`privstack.*` for core, `community.*` for third-party) |
| `name` | string | Yes | Display name shown in sidebar and settings |
| `description` | string | Yes | One-line summary |
| `version` | string | Yes | SemVer version |
| `author` | string | Yes | Author name |
| `icon` | string | No | Lucide icon name |
| `navigation_order` | number | Yes | Sidebar position (see ranges below) |
| `category` | string | Yes | One of: `productivity`, `security`, `communication`, `information`, `utility`, `extension` |
| `can_disable` | bool | Yes | Whether users can disable this plugin |
| `is_experimental` | bool | Yes | Show experimental badge |

#### Navigation Order Ranges

| Range | Purpose |
|-------|---------|
| 100–199 | Primary plugins (Notes, Tasks, Calendar) |
| 200–299 | Secondary plugins (Contacts, Bookmarks) |
| 300–399 | Utility plugins (RSS, Timer, Calculator) |
| 1000+ | Third-party / community plugins |

### 9.2 `schemas.json`

Array of entity schema declarations. Must agree with `Plugin::entity_schemas()`.

```json
[
  {
    "entity_type": "feed",
    "indexed_fields": [
      {
        "field_path": "/title",
        "field_type": "text",
        "searchable": true,
        "vector_dim": null,
        "enum_options": null
      },
      {
        "field_path": "/tags",
        "field_type": "tag",
        "searchable": true,
        "vector_dim": null,
        "enum_options": null
      },
      {
        "field_path": "/last_fetched",
        "field_type": "datetime",
        "searchable": false,
        "vector_dim": null,
        "enum_options": null
      }
    ],
    "merge_strategy": "lww-per-field"
  }
]
```

#### All 13 Field Types

| `field_type` value | Description | Special Fields |
|-------------------|-------------|----------------|
| `"text"` | UTF-8 string | — |
| `"tag"` | String array, always searchable | — |
| `"datetime"` | Unix epoch or ISO 8601 timestamp | — |
| `"number"` | Integer (i64) | — |
| `"boolean"` | true/false | — |
| `"vector"` | Float array for embeddings | `vector_dim` required |
| `"counter"` | CRDT counter | — |
| `"relation"` | Entity ID foreign key | — |
| `"decimal"` | Fixed-point number | — |
| `"json"` | Arbitrary JSON blob | — |
| `"enumeration"` | Fixed string options | `enum_options` required |
| `"geo-point"` | Latitude/longitude pair | — |
| `"duration"` | Time duration in milliseconds | — |

#### Merge Strategies

| Value | Behavior |
|-------|----------|
| `"lww-document"` | Last writer wins — entire document replaced |
| `"lww-per-field"` | Last writer wins per indexed field (recommended) |
| `"custom"` | Plugin handles conflict resolution |

### 9.3 `template.json`

A FluidUI declarative template. The host evaluates this against the JSON returned by `TemplateDataProvider::get_view_data()` using Mustache-style `{{variable}}` interpolation.

#### Layout Components

| Type | Description | Key Properties |
|------|-------------|----------------|
| `stack` | Vertical stack | `spacing`, `children` |
| `multi_pane` | Side-by-side panes | `panes: [{ id, width?, flex?, content }]` |
| `scroll` | Scrollable container | `child` |
| `page` | Padded content page | `children` |
| `spacer` | Vertical gap | `height` |

#### Input Components

| Type | Description | Key Properties |
|------|-------------|----------------|
| `text_input` | Text field with submit | `placeholder`, `on_submit_command`, `submit_button_label` |
| `tab_bar` | Tab selector | `command`, `tabs: [{ label, value, is_active }]`, `fill` |

#### Display Components

| Type | Description | Key Properties |
|------|-------------|----------------|
| `text` | Text block | `value`, `style` (`heading`, `muted`, etc.) |
| `section_header` | Section label | `text` |
| `separator` | Horizontal line | — |
| `empty_state` | Placeholder message | `message` |
| `html_content` | Rendered HTML | `html` |

#### Composite Components

| Type | Description | Key Properties |
|------|-------------|----------------|
| `toolbar` | Top bar with title + actions | `title`, `subtitle`, `actions` |
| `status_bar` | Bottom bar with stats | `items: [{ label, value }]`, `status_message` |
| `detail_header` | Item detail header | `title`, `metadata`, `actions` |
| `card` | Card container | `padding`, `children` |
| `feed_item` | Feed list row | `id`, `title`, `unread_count`, `command`, `context_menu` |
| `article_item` | Article list row | `id`, `title`, `date`, `starred`, `unread`, `command`, `context_menu` |

#### Button Components

| Type | Description | Key Properties |
|------|-------------|----------------|
| `button` | Standard button | `label`, `command`, `args` |
| `icon_button` | Icon-only button | `icon`, `label`, `command`, `variant` |

#### Control Flow

| Directive | Description | Example |
|-----------|-------------|---------|
| `$if` / `$then` / `$else` | Conditional rendering | `{ "$if": "has_articles", "$then": { ... }, "$else": { ... } }` |
| `$for` / `$in` / `$template` / `$empty` | List iteration | `{ "$for": "item", "$in": "items", "$template": { ... }, "$empty": { ... } }` |

#### Context Menus

Any component with a `command` can include a `context_menu` array:

```json
"context_menu": [
  { "label": "Open", "command": "open_item" },
  { "label": "Rename", "command": "rename", "icon": "Edit" },
  { "type": "separator" },
  { "label": "Delete", "command": "delete", "variant": "danger" }
]
```

### 9.4 `command_palettes.json`

Array of custom command palettes. Each palette has a searchable list of items.

```json
[
  {
    "id": "add_block",
    "title": "Add Block",
    "placeholder": "Search block types...",
    "shortcut": "ctrl+/",
    "items": [
      {
        "id": "paragraph",
        "name": "Paragraph",
        "description": "Plain text",
        "icon": "Type",
        "keywords": "text p plain",
        "command": "add_block",
        "args": "{\"block_type\":\"paragraph\"}"
      }
    ]
  }
]
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Palette identifier |
| `title` | string | Palette header text |
| `placeholder` | string | Search input placeholder |
| `shortcut` | string | Keyboard shortcut to open (e.g., `"ctrl+/"`) |
| `items[].id` | string | Item identifier |
| `items[].name` | string | Display name |
| `items[].description` | string | One-line description |
| `items[].icon` | string | Lucide icon name |
| `items[].keywords` | string | Space-separated search terms |
| `items[].command` | string | Command name sent to `handle_command` |
| `items[].args` | string | JSON args passed to the command |

---

## 10. Packaging (.ppk)

Plugins are distributed as `.ppk` (PrivStack Plugin Package) files. See [plugin-package-format.md](plugin-package-format.md) for the full specification.

### Summary

A `.ppk` is a ZIP archive:

```
plugin.ppk (ZIP)
├── manifest.toml       # Metadata, permissions, schemas
├── plugin.wasm         # Wasm Component Model binary
├── signature.bin       # Ed25519 detached signature (optional)
├── icon.png            # 256x256 plugin icon (optional)
├── README.md           # Documentation (optional)
└── views/              # Declarative UI definitions (optional)
    ├── main.json
    └── settings.json
```

### Build & Sign

```rust
let ppk_bytes = PackageBuilder::new(manifest)
    .wasm(wasm_bytes)
    .readme(readme_bytes)
    .add_view("main", view_json)
    .sign(&keypair.signing_key)
    .build()?;
```

---

## 11. Permissions & Sandbox

### 9 Permission Types

| Permission | Description | Tier |
|-----------|-------------|------|
| `EntityCrud` | Create, read, update, delete entities | 1 (auto) |
| `EntityQuery` | Query/search entities | 1 (auto) |
| `ViewState` | Plugin view state (UI preferences) | 1 (auto) |
| `CommandPalette` | Register command palette entries | 1 (auto) |
| `VaultAccess` | Encrypted vault read/write | 2 (JIT) |
| `CrossPluginLink` | Link entities across plugins | 2 (JIT) |
| `DialogDisplay` | Show confirmation/file dialogs | 2 (JIT) |
| `TimerAccess` | Use timer/scheduling APIs | 2 (JIT) |
| `NetworkAccess` | Outbound HTTP requests | 3 (install) |

### 3 Trust Tiers

| Tier | Signing | Access |
|------|---------|--------|
| **Core** | Signed with PrivStack key | Full access to all APIs |
| **Verified** | Signed with registered developer key | Declared permissions only |
| **Community** | Unsigned or unknown key | Restricted — no vault, no network |

### Resource Limits

The Wasm sandbox enforces:

- **Memory**: Bounded linear memory (host-configurable, default 256 MB)
- **CPU**: Fuel-based execution limits per call
- **Stack**: Wasm call stack depth limit
- **Network**: Host proxies all requests; domain allowlisting enforced
- **Storage**: Entity CRUD scoped to declared entity types only — plugins cannot read other plugins' data (unless using the `agent` world with explicit permissions)

---

## 12. Complete Example: RSS Reader

A fully annotated RSS/Atom feed reader plugin. This is a real plugin shipping with PrivStack.

### `Cargo.toml`

```toml
[package]
name = "privstack-plugin-rss"
description = "RSS/Atom feed reader plugin for PrivStack (Wasm sandbox)"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
privstack-plugin-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
quick-xml = { version = "0.36", features = ["serialize"] }
rss = "2.0"
wit-bindgen = { version = "0.38", default-features = false, features = ["macros"] }

[dev-dependencies]
pretty_assertions = "1.4"
```

### `metadata.json`

```json
{
  "id": "privstack.rss",
  "name": "Pulse",
  "description": "RSS/Atom feed reader with reader mode",
  "version": "1.0.0",
  "author": "PrivStack",
  "icon": "Rss",
  "navigation_order": 350,
  "category": "utility",
  "can_disable": true,
  "is_experimental": false
}
```

### `schemas.json`

```json
[
  {
    "entity_type": "feed",
    "indexed_fields": [
      { "field_path": "/title", "field_type": "text", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/url", "field_type": "text", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/description", "field_type": "text", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/tags", "field_type": "tag", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/last_fetched", "field_type": "datetime", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/created_at", "field_type": "datetime", "searchable": false, "vector_dim": null, "enum_options": null }
    ],
    "merge_strategy": "lww-per-field"
  },
  {
    "entity_type": "feed_item",
    "indexed_fields": [
      { "field_path": "/title", "field_type": "text", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/summary", "field_type": "text", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/author", "field_type": "text", "searchable": true, "vector_dim": null, "enum_options": null },
      { "field_path": "/url", "field_type": "text", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/feed_id", "field_type": "relation", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/read", "field_type": "boolean", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/starred", "field_type": "boolean", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/published_at", "field_type": "datetime", "searchable": false, "vector_dim": null, "enum_options": null },
      { "field_path": "/fetched_at", "field_type": "datetime", "searchable": false, "vector_dim": null, "enum_options": null }
    ],
    "merge_strategy": "lww-per-field"
  },
  {
    "entity_type": "plugin_prefs",
    "indexed_fields": [],
    "merge_strategy": "lww-per-field"
  }
]
```

### `template.json`

```json
{
  "type": "stack",
  "spacing": 0,
  "children": [
    {
      "type": "toolbar",
      "title": "{{metadata.name}}",
      "subtitle": "{{metadata.subtitle}}",
      "actions": [
        { "type": "icon_button", "icon": "↻", "label": "Refresh", "command": "refresh_feeds", "variant": "default" },
        { "type": "icon_button", "icon": "✓", "label": "Mark All Read", "command": "mark_all_read", "variant": "default" }
      ]
    },
    {
      "type": "multi_pane",
      "panes": [
        {
          "id": "feeds", "width": 280,
          "content": {
            "type": "scroll",
            "child": {
              "type": "stack", "spacing": 0,
              "children": [
                { "type": "section_header", "text": "Filters" },
                {
                  "type": "tab_bar", "command": "set_filter", "fill": true,
                  "tabs": [
                    { "label": "All", "value": "all", "is_active": "{{is_all_filter}}" },
                    { "label": "Unread", "value": "unread", "is_active": "{{is_unread_filter}}" },
                    { "label": "Starred", "value": "starred", "is_active": "{{is_starred_filter}}" }
                  ]
                },
                { "type": "spacer", "height": 8 },
                { "type": "section_header", "text": "Add Feed" },
                { "type": "text_input", "placeholder": "https://example.com/feed.xml", "on_submit_command": "add_feed", "submit_button_label": "Add" },
                { "type": "spacer", "height": 12 },
                { "type": "section_header", "text": "Feeds" },
                {
                  "$for": "feed", "$in": "feeds",
                  "$template": {
                    "type": "feed_item", "id": "{{feed.id}}", "title": "{{feed.title}}", "unread_count": "{{feed.unread_count}}",
                    "command": "select_feed",
                    "context_menu": [
                      { "label": "Select", "command": "select_feed" },
                      { "label": "Refresh", "command": "refresh_single_feed", "icon": "↻" },
                      { "type": "separator" },
                      { "label": "Delete", "command": "remove_feed", "variant": "danger" }
                    ]
                  },
                  "$empty": { "type": "text", "value": "No feeds yet", "style": "muted" }
                }
              ]
            }
          }
        },
        {
          "id": "articles", "width": 320,
          "content": {
            "$if": "has_articles",
            "$then": {
              "type": "scroll",
              "child": {
                "type": "stack", "spacing": 0,
                "children": [
                  {
                    "$for": "article", "$in": "filtered_articles",
                    "$template": {
                      "type": "article_item", "id": "{{article.id}}", "title": "{{article.title}}",
                      "date": "{{article.author | default: }}", "starred": "{{article.starred}}", "unread": "{{article.unread}}",
                      "command": "select_article",
                      "context_menu": [
                        { "label": "Open", "command": "select_article" },
                        { "label": "Toggle Star", "command": "toggle_starred", "icon": "☆" },
                        { "label": "Mark Read", "command": "mark_read", "icon": "✓" }
                      ]
                    }
                  }
                ]
              }
            },
            "$else": { "type": "empty_state", "message": "No articles match your filters" }
          }
        },
        {
          "id": "content", "flex": true,
          "content": {
            "$if": "selected_article",
            "$then": {
              "type": "scroll",
              "child": {
                "type": "stack", "spacing": 0,
                "children": [
                  {
                    "type": "detail_header", "title": "{{selected_article.title}}",
                    "metadata": [
                      { "$if": "selected_article.author_display", "$then": { "value": "{{selected_article.author_display}}" } }
                    ],
                    "actions": [
                      { "type": "button", "label": "{{selected_article.star_label}}", "command": "toggle_starred", "args": "{{selected_article.star_args}}" },
                      { "type": "button", "label": "Mark Read", "command": "mark_read", "args": "{{selected_article.read_args}}" }
                    ]
                  },
                  { "type": "separator" },
                  {
                    "type": "page",
                    "children": [
                      { "type": "card", "padding": 16, "children": [
                        { "type": "text", "value": "{{selected_article.summary}}", "style": "muted" }
                      ]},
                      { "type": "html_content", "html": "{{selected_article.content_html}}" }
                    ]
                  }
                ]
              }
            },
            "$else": { "type": "empty_state", "message": "Select an article to read" }
          }
        }
      ]
    },
    {
      "type": "status_bar",
      "items": [
        { "label": "Feeds", "value": "{{feed_count}}" },
        { "label": "Articles", "value": "{{article_count}}" },
        { "label": "Unread", "value": "{{unread_count}}" }
      ],
      "status_message": "{{status_message}}"
    }
  ]
}
```

### `src/models.rs`

```rust
use serde::{Deserialize, Serialize};

/// An RSS/Atom feed subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssFeed {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub site_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub custom_title: Option<String>,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default)]
    pub last_fetched: Option<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
}

impl RssFeed {
    /// Display title: custom title if set, otherwise feed title.
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(&self.title)
    }
}

/// An article from an RSS/Atom feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssArticle {
    pub id: String,
    pub feed_id: String,
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub published_at: Option<i64>,
    #[serde(default)]
    pub fetched_at: Option<i64>,
}
```

### `src/state.rs`

```rust
use crate::models::{RssArticle, RssFeed};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode { All, Unread, Starred, Today, LastWeek }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode { NewestFirst, OldestFirst, ByTitle, ByFeed }

/// Internal mutable state for the RSS plugin.
pub struct RssState {
    pub feeds: Vec<RssFeed>,
    pub articles: Vec<RssArticle>,
    pub filtered_articles: Vec<usize>,       // indices into `articles`
    pub selected_feed_id: Option<String>,
    pub selected_article_id: Option<String>,
    pub filter_mode: FilterMode,
    pub sort_mode: SortMode,
    pub search_query: String,
    pub total_unread_count: u32,
    pub total_starred_count: u32,
    pub today_count: u32,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub needs_reload: bool,
}

impl RssState {
    pub fn new() -> Self { /* ... */ }
    pub fn clear(&mut self) { /* ... */ }
    pub fn update_statistics(&mut self) { /* ... */ }
    pub fn apply_filters(&mut self) { /* ... */ }
    pub fn to_view_data_json(&self) -> String { /* ... */ }
    pub fn filtered_article_ids(&self) -> Vec<String> { /* ... */ }
}
```

*(Full implementation in `PrivStack-Plugins/wasm/privstack-plugin-rss/src/state.rs`.)*

### `src/lib.rs`

```rust
mod models;
mod state;

use models::{RssArticle, RssFeed};
use privstack_plugin_sdk::prelude::*;
use state::{FilterMode, RssState, SortMode};

// Generate WIT exports with LinkableItemProvider + TemplateDataProvider capabilities.
privstack_plugin_sdk::privstack_wasm_export!(RssPlugin, [LinkableItemProvider, TemplateDataProvider]);

pub struct RssPlugin {
    state: RssState,
}

impl Default for RssPlugin {
    fn default() -> Self {
        Self { state: RssState::new() }
    }
}

impl Plugin for RssPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "privstack.rss".into(),
            name: "RSS".into(),
            description: "RSS/Atom feed reader with reader mode".into(),
            version: "1.0.0".into(),
            author: "PrivStack".into(),
            icon: Some("Rss".into()),
            navigation_order: 350,
            category: PluginCategory::Utility,
            can_disable: true,
            is_experimental: false,
        }
    }

    fn entity_schemas(&self) -> Vec<EntitySchema> {
        vec![
            EntitySchema {
                entity_type: "feed".into(),
                indexed_fields: vec![
                    IndexedField::text("/title", true),
                    IndexedField::text("/url", false),
                    IndexedField::text("/description", true),
                    IndexedField::tag("/tags"),
                    IndexedField::datetime("/last_fetched"),
                    IndexedField::datetime("/created_at"),
                ],
                merge_strategy: MergeStrategy::LwwPerField,
            },
            EntitySchema {
                entity_type: "feed_item".into(),
                indexed_fields: vec![
                    IndexedField::text("/title", true),
                    IndexedField::text("/summary", true),
                    IndexedField::text("/author", true),
                    IndexedField::text("/url", false),
                    IndexedField::relation("/feed_id"),
                    IndexedField::boolean("/read"),
                    IndexedField::boolean("/starred"),
                    IndexedField::datetime("/published_at"),
                    IndexedField::datetime("/fetched_at"),
                ],
                merge_strategy: MergeStrategy::LwwPerField,
            },
            EntitySchema {
                entity_type: "plugin_prefs".into(),
                indexed_fields: vec![],
                merge_strategy: MergeStrategy::LwwPerField,
            },
        ]
    }

    fn navigation_item(&self) -> Option<NavigationItem> {
        Some(NavigationItem {
            id: "Rss".into(),
            display_name: "Pulse".into(),
            subtitle: Some("Stay informed".into()),
            icon: Some("Rss".into()),
            tooltip: Some("Pulse - Stay informed (Cmd+9)".into()),
            order: 350,
            show_badge: self.state.total_unread_count > 0,
            badge_count: self.state.total_unread_count,
            shortcut_hint: Some("Cmd+9".into()),
        })
    }

    fn commands(&self) -> Vec<CommandDefinition> {
        vec![
            CommandDefinition {
                name: "RSS: Refresh All Feeds".into(),
                description: "Fetch new articles from all subscribed feeds".into(),
                keywords: "rss refresh update sync".into(),
                category: "RSS".into(),
                icon: Some("RefreshCw".into()),
            },
            // ... more commands
        ]
    }

    fn initialize(&mut self) -> bool { true }

    fn activate(&mut self) {
        // Load feeds and articles from the entity store via sdk::send
        self.state.feeds = load_feeds_from_store();
        self.state.articles = load_articles_from_store();
        self.state.apply_filters();
        self.state.update_statistics();
    }

    fn handle_command(&mut self, name: &str, args: &str) -> String {
        match name {
            "add_feed" => self.handle_add_feed(args),
            "remove_feed" => self.handle_remove_feed(args),
            "select_feed" => self.handle_select_feed(args),
            "select_article" => self.handle_select_article(args),
            "toggle_starred" => self.handle_toggle_starred(args),
            "mark_read" => self.handle_mark_read(args),
            "mark_all_read" => self.handle_mark_all_read(),
            "set_filter" => self.handle_set_filter(args),
            "refresh_feeds" => self.handle_refresh_feeds(),
            // ... more commands
            _ => r#"{"success":false,"error":"unknown command"}"#.into(),
        }
    }
}

impl LinkableItemProvider for RssPlugin {
    fn link_type(&self) -> &str { "rss_article" }
    fn link_type_display_name(&self) -> &str { "RSS Articles" }
    fn link_type_icon(&self) -> &str { "Rss" }

    fn search_items(&self, query: &str, max_results: u32) -> Vec<LinkableItem> {
        let query_lower = query.to_lowercase();
        self.state.articles.iter()
            .filter(|a| query.is_empty() || a.title.to_lowercase().contains(&query_lower))
            .take(max_results as usize)
            .map(|a| LinkableItem {
                id: a.id.clone(),
                link_type: "rss_article".into(),
                title: a.title.clone(),
                subtitle: a.author.clone(),
                icon: Some("Rss".into()),
                modified_at: a.published_at.unwrap_or(0) as u64,
            })
            .collect()
    }

    fn get_item_by_id(&self, id: &str) -> Option<LinkableItem> {
        self.state.articles.iter().find(|a| a.id == id).map(|a| LinkableItem {
            id: a.id.clone(),
            link_type: "rss_article".into(),
            title: a.title.clone(),
            subtitle: a.author.clone(),
            icon: Some("Rss".into()),
            modified_at: a.published_at.unwrap_or(0) as u64,
        })
    }
}

impl TemplateDataProvider for RssPlugin {
    fn get_view_data(&self) -> String {
        self.state.to_view_data_json()
    }
}
```

*(Full implementation with all command handlers, RSS parsing, and network fetching in `PrivStack-Plugins/wasm/privstack-plugin-rss/src/lib.rs`.)*
