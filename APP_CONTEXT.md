# PrivStack — Application Context

PrivStack is a local-first, privacy-focused personal productivity suite. All data is stored on the user's device using a Rust core with DuckDB persistence. The desktop shell is built with Avalonia (C#/.NET 9) and uses a plugin architecture — each functional module is an isolated plugin that communicates through the SDK message bus.

## Core Architecture

- **Rust Core**: Handles persistence (DuckDB), encryption, FFI, and sync relay
- **Desktop Shell**: Avalonia-based MVVM application with plugin lifecycle management
- **SDK**: `PrivStack.Sdk` defines the plugin boundary — all plugins communicate exclusively through `Host.Sdk.SendAsync()` and capability interfaces
- **Plugin Isolation**: Plugins never reference the desktop shell or other plugins directly. Cross-plugin features use capability interfaces discovered via `IPluginRegistry`

## Shell-Level Features

### Knowledge Graph (Built-in Plugin)
A dual-mode visualization of all entities and their relationships across every plugin.

**Knowledge Graph Tab (Force-Directed)**
- Aggregates nodes from all plugins implementing `IGraphDataProvider`
- Node types: pages, tasks, contacts, events, journal entries, habits, goals, files, snippets, RSS articles, web clips, whiteboards
- Edge types: wiki-links (`[[Page Name]]`), parent-child, project membership, company/group associations, tag connections
- Wiki-link parsing extracts `[[...]]` references from text content across all plugins
- Tag synthesis creates virtual tag nodes connecting entities that share tags
- Filtering: by node type, tag, link count, date range, orphan mode, search text
- Layout modes: force-directed (with tunable physics) and solar system (orbital hierarchy)
- Local view: center on any entity with configurable hop depth (1-5)
- Global view: entire knowledge graph

**Embedding Space Tab (3D Semantic Visualization)**
- Fetches 768-dimensional RAG embeddings from the Rust core
- Projects to 3D via random projection for interactive exploration
- K-nearest neighbor edges show semantic similarity between content chunks
- Configurable: similarity threshold, max points, max neighbors, auto-rotate
- Entity type visibility toggles (notes, tasks, contacts, events, journal, snippets, RSS, files)
- Click a point to see its nearest semantic neighbors in the info panel

### Dashboard (Built-in Plugin)
System overview and plugin management center.

**Overview Tab**
- Official plugin catalog with install/update/uninstall/reload/toggle activation
- System metrics: app shell size, plugin binaries total, data storage total, memory usage
- Plugin marketplace with search, category filtering, release stage badges (alpha/beta/release)
- Per-workspace plugin activation (enable/disable plugins for current workspace)

**Data Tab**
- Per-plugin data storage breakdown with entity counts and actual disk sizes
- Database diagnostics: table-level detail (row counts, estimated vs actual sizes, backing mode)
- Maintenance operations:
  - Database maintenance (cleanup orphaned rows, checkpoint)
  - Orphan entity cleanup (find/delete entities from uninstalled plugins)
  - Database compaction (reclaim fragmented space, reports before/after size)
  - Metadata validation (clean orphaned metadata records)

### Other Shell Features
- **Universal Search (Cmd+K)**: Global search across all plugins via command palette
- **Command Palette**: Aggregates commands from all `ICommandProvider` plugins plus quick actions
- **Quick Actions**: Modal overlays triggered by keyboard shortcuts (e.g., Cmd+T for new task, Cmd+N for sticky note). Plugins declare shortcuts via `QuickActionDescriptor`
- **Info Panel (Right Sidebar)**: Shows active entity details, backlinks, cross-plugin references
- **Backlink Service**: Reverse-link index across all `ILinkableItemProvider` plugins
- **Cloud Sync**: Bi-directional workspace synchronization with conflict resolution
- **AI Services (Duncan)**: See AI & Intent System section below for full details
- **Reminders**: Aggregates from all `IReminderProvider` plugins into system notifications
- **Theme Management**: Light/dark themes with custom color palette editor
- **Speech/Audio**: Recording + Whisper STT for voice input
- **Local HTTP API**: Opt-in Kestrel server on `127.0.0.1:9720` for programmatic access. Plugins declare routes via `IApiProvider`; the shell hosts the server, handles API key auth (`X-API-Key` / `Bearer`), and routes requests. Shell endpoints: `/api/v1/status` (health, no auth), `/api/v1/routes` (route listing). Enabled via `ApiEnabled` in settings.
- **Multi-Workspace**: Separate data directories per workspace with independent plugin activation
- **Backup & Recovery**: Workspace snapshots with restore capability
- **Master Password**: Encrypted vault for sensitive data (credentials, files)

## Plugin Ecosystem

Each plugin implements a subset of SDK capability interfaces to participate in cross-cutting features:

| Capability | Purpose |
|---|---|
| `IGraphDataProvider` | Contribute nodes/edges to the knowledge graph |
| `ILinkableItemProvider` | Enable cross-plugin entity linking and search |
| `IDataMetricsProvider` | Report storage metrics to the dashboard |
| `IReminderProvider` | Contribute reminders to the notification system |
| `ISeedDataProvider` | Generate demo data and support wipe operations |
| `IShareableBehavior` | Export/share functionality |
| `ICommandProvider` | Register entries in the global command palette |
| `IDeepLinkTarget` | URI-based navigation (privstack:// links) |
| `IPluginDataSourceProvider` | Expose data for analytics and cross-plugin queries |
| `IIntentProvider` | Declare AI intents for natural language interaction |
| `IQuickActionProvider` | Register keyboard-shortcut-triggered quick actions |
| `IIndexableContentProvider` | Contribute content to the RAG vector index |
| `IVaultConsumer` | Store encrypted credentials in the system vault |
| `IConnectionConsumer` | Declare required OAuth connections |
| `IStorageProvider` | Provide file storage operations |
| `IApiProvider` | Declare local HTTP API routes for programmatic access |

## Available Plugins

| Plugin | ID | Description |
|---|---|---|
| Notes | privstack.notes | Block editor with wiki links, page hierarchy, sticky notes, templates, GitHub wiki sync |
| Tasks | privstack.tasks | Task management with list/kanban/timeline/calendar views, time tracking, GitHub Issues sync |
| Calendar | privstack.calendar | Event management with month/week/day/agenda views, recurrence, CalDAV/ICS sync |
| Contacts | privstack.contacts | CRM with contacts, companies, groups, interactions, org charts, import (vCard/CSV) |
| Journal | privstack.journal | Daily journaling with templates, mood tracking, streaks, calendar heatmap, insights |
| Finance | privstack.finance | Double-entry bookkeeping with ZBB/envelope budgeting, reconciliation, reports |
| Habits | privstack.habits | Habit tracking with streaks and goal management with milestones and analytics |
| Mail | privstack.email | IMAP/SMTP email client with multi-account, OAuth, threading, reader mode |
| RSS | privstack.rss | RSS/Atom feed reader with reader mode, OPML import/export |
| Snippets | privstack.snippets | Code snippet manager with syntax highlighting (23 languages), collections |
| Data | privstack.data | Tabular dataset browser with SQL query engine, CSV/Notion import |
| Files | privstack.files | Encrypted file vault + media library with folder organization |
| WebClips | privstack.webclips | Web clipping, read-it-later queue, browsing history, transcription |
| Canvas | privstack.canvas | Infinite canvas whiteboards with cross-plugin entity references |

Each plugin has its own `PLUGIN_CONTEXT.md` with detailed feature documentation in its directory.

## AI & Intent System (Duncan)

Duncan is PrivStack's built-in AI assistant, accessible via the AI tray (star icon in the top-right corner).

### Interacting with Duncan
- Click the star icon to open the AI tray, type in the "Ask Duncan..." box
- Duncan can answer questions about your data, summarize content, brainstorm, and explain features
- Duncan has access to your workspace data via RAG (semantic search over embeddings)
- Conversation history is preserved and can be resumed
- Duncan learns preferences across conversations via AI memory

### AI Providers
Configure in Settings > AI:
- **OpenAI** (GPT-4o, GPT-4, GPT-3.5-turbo) — requires API key
- **Anthropic** (Claude 3.5 Sonnet, Claude 3 Opus/Haiku) — requires API key
- **Google Gemini** (Gemini Pro, Gemini Flash) — requires API key
- **Local LLaMA** — fully offline, on-device inference with token streaming

### Intent System
Intents are AI-powered actions that let Duncan create, query, and manage data across all plugins via natural language.

**How intents work:**
1. You type a natural language request (e.g., "Create a task to review the budget by Friday")
2. The Intent Engine classifies your message against all registered intents
3. Slots (parameters) are extracted from your message
4. The owning plugin executes the intent
5. Duncan confirms and links to the created entity

**Intent signals:** Duncan proactively monitors plugin signals and suggests relevant actions as cards in the AI tray.

### Available Intents

| Plugin | Intent ID | Description |
|---|---|---|
| Notes | `notes.create_note` | Create a note page (title, content, tags) |
| Tasks | `tasks.create_task` | Create a task (title, description, priority, due_date, tags) |
| Calendar | `calendar.create_event` | Create event (title, date, time, duration, location, description) |
| Contacts | `contacts.create_contact` | Create contact (name, email, phone, company, notes) |
| Journal | `journal.create_entry` | Create journal entry (title, content, mood, tags) |
| Finance | `finance.create_transaction` | Create transaction (payee, amount, account, date, memo) |
| Finance | `finance.check_balance` | Check account balance |
| Finance | `finance.check_budget` | Check category budget status |
| Finance | `finance.transfer_between_categories` | Move budget money between categories |
| Finance | `finance.get_monthly_summary` | Monthly income/expense summary |
| Finance | `finance.get_spending_breakdown` | Spending by category for date range |
| Finance | `finance.get_budget_health` | Budget health check |
| Finance | `finance.get_financial_trends` | Trends over time |
| Finance | `finance.get_account_overview` | All account balances |
| Finance | `finance.suggest_from_receipt` | Suggest transaction from receipt text |
| Habits | `habits.log_habit` | Log a habit completion |
| Habits | `habits.create_habit` | Create a new habit |
| Habits | `habits.create_goal` | Create a goal with milestones |
| Email | `email.draft_email` | Draft email (to, subject, body) |
| Snippets | `snippets.save_snippet` | Save code snippet (title, language, code) |
| RSS | `rss.add_feed` | Subscribe to RSS feed (url, name, category) |

### Content Suggestions
Plugins can push AI-generated suggestion cards into Duncan's tray (e.g., rewritten task descriptions, email summaries). Cards appear alongside intent suggestions and can be acted on or dismissed.

### RAG (Retrieval-Augmented Generation)
- All plugins implementing `IIndexableContentProvider` contribute content chunks to the vector index
- Chunks are embedded as 768-dimensional vectors stored in the Rust core
- When you ask Duncan a question, relevant chunks are retrieved via semantic similarity and injected as context
- This lets Duncan answer questions about your specific data without sending it to the cloud (when using local models)
