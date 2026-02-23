using System.Security.Cryptography;
using System.Text;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Shell-level RAG content provider that indexes global features, shortcuts,
/// and capabilities so the AI can answer questions about app-wide functionality.
/// Registered with the CapabilityBroker (not a plugin) so RagIndexService discovers it.
/// </summary>
internal sealed class ShellContentProvider : IIndexableContentProvider
{
    private const string ShellPluginId = "privstack.desktop";

    public Task<IndexableContentResult> GetIndexableContentAsync(
        IndexableContentRequest request, CancellationToken ct = default)
    {
        var chunks = new List<ContentChunk>();

        // ── Global Keyboard Shortcuts ────────────────────────────────────
        chunks.Add(MakeChunk("shell-shortcuts", "Global Keyboard Shortcuts",
            """
            PrivStack global keyboard shortcuts and navigation:
            - Cmd+K / Ctrl+K: Open Universal Search (search across all plugins, navigate anywhere)
            - Cmd+I / Ctrl+I: Toggle the Info Panel (right sidebar showing entity details and backlinks)
            - Cmd+M / Ctrl+M: Toggle Speech Recording (Whisper speech-to-text, inserts at cursor)
            - Cmd+\ / Ctrl+\: Toggle sidebar collapsed/expanded
            - Cmd+1 through Cmd+9 / Ctrl+1-9: Switch to plugin tab by position (1=first plugin, 2=second, etc.)
            - Escape: Close active overlay, panel, or modal (AI tray, info panel, quick action, search dropdown)
            - Cmd+T / Ctrl+T: Quick Task (creates a new task via overlay)
            - Cmd+N / Ctrl+N: Quick Sticky Note (creates a new sticky note via overlay)
            - Cmd+E / Ctrl+E: Quick Event (creates a new calendar event via overlay)
            - Cmd+H / Ctrl+H: Quick Log Habit (log a habit entry via overlay)
            - Cmd+B / Ctrl+B: Quick Transaction (add a financial transaction via overlay)
            Plugin-specific shortcuts are declared by plugins via QuickActionDescriptor and resolved generically by the shell.
            """));

        // ── Universal Search & Command Palette ───────────────────────────
        chunks.Add(MakeChunk("shell-search", "Universal Search & Command Palette",
            """
            Universal Search (Cmd+K): A unified search interface that searches across all plugins simultaneously.
            Type to search for notes, tasks, contacts, events, journal entries, files, snippets, RSS articles, web clips, habits, goals, and financial accounts.
            Results show the entity type, title, and plugin source. Click a result to navigate directly to that entity.
            The Command Palette is integrated into Universal Search — type ">" to switch to command mode, which shows actions from all ICommandProvider plugins (e.g., "New Page", "Import CSV", "Toggle Dark Mode").
            Quick Actions also appear in the command palette with their keyboard shortcut hints.
            """));

        // ── Knowledge Graph ──────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-knowledge-graph", "Knowledge Graph",
            """
            Knowledge Graph: A built-in visualization of all entities and their relationships across every plugin.
            Access it from the Knowledge Graph tab in the sidebar navigation.

            Force-Directed Graph Tab:
            - Aggregates nodes from all plugins implementing IGraphDataProvider
            - Shows nodes for pages, tasks, contacts, events, journal entries, habits, goals, files, snippets, RSS articles, web clips, whiteboards, and financial accounts/budgets
            - Edge types: wiki-links ([[Page Name]]), parent-child, project membership, company/group associations, tag connections
            - Filtering: by node type, tag, link count, date range, orphan mode, search text
            - Layout modes: force-directed (tunable physics) and solar system (orbital hierarchy)
            - Local view: center on any entity with configurable hop depth (1-5)
            - Click any node to see its details in the info panel

            Embedding Space Tab (3D Semantic Visualization):
            - Fetches 768-dimensional RAG embeddings and projects to 3D via random projection
            - K-nearest neighbor edges show semantic similarity between content chunks
            - Configurable: similarity threshold, max points, max neighbors, auto-rotate
            - Entity type visibility toggles
            - Click a point to see its nearest semantic neighbors
            """));

        // ── Dashboard ────────────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-dashboard", "Dashboard & Plugin Management",
            """
            Dashboard: The system overview and plugin management center. Access via the Dashboard tab.

            Overview Tab:
            - Official plugin catalog with install, update, uninstall, reload, and toggle activation
            - System metrics: app shell size, plugin binaries total, data storage total, memory usage
            - Plugin marketplace with search, category filtering, release stage badges (alpha/beta/release)
            - Per-workspace plugin activation (enable/disable plugins for current workspace)

            Data Tab:
            - Per-plugin data storage breakdown with entity counts and actual disk sizes
            - Database diagnostics: table-level detail (row counts, estimated vs actual sizes, backing mode)
            - Maintenance operations: database maintenance, orphan entity cleanup, database compaction, metadata validation
            """));

        // ── AI Services ──────────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-ai", "AI Services & Duncan",
            """
            AI Services in PrivStack (assistant name: Duncan):
            Duncan is PrivStack's built-in AI assistant, accessible via the AI tray (star icon in the top-right corner).

            Features:
            - Free-form chat: Ask Duncan questions about your data, get summaries, brainstorm ideas
            - Intent recognition: Duncan monitors signals from plugins and suggests actions (e.g., "Create a task from this email", "Add a calendar event for this meeting")
            - Content suggestions: Plugins can push rich content cards into the AI tray
            - RAG search: Duncan searches your entire knowledge base using semantic embeddings to find relevant context

            AI Provider Support:
            - OpenAI (GPT-4, GPT-3.5)
            - Anthropic (Claude)
            - Google Gemini
            - Local LLaMA models (runs on-device, fully offline with token streaming)

            Configuration: Go to Settings (gear icon) to configure API keys, select the active provider, or download a local model.
            The AI tray shows a notification balloon when Duncan has insights. Click the star icon or the balloon to open the tray.
            """));

        // ── Info Panel & Backlinks ───────────────────────────────────────
        chunks.Add(MakeChunk("shell-info-panel", "Info Panel & Backlinks",
            """
            Info Panel (Cmd+I): A right sidebar that shows details about the currently selected entity.
            - Displays entity properties, metadata, and cross-plugin relationships
            - Shows backlinks: all entities across all plugins that reference the selected entity via wiki-links ([[Entity Name]])
            - The backlink service maintains a reverse-link index across all ILinkableItemProvider plugins
            - Click any backlink to navigate to the referencing entity
            - Drag the left edge to resize the panel
            """));

        // ── Cloud Sync & Multi-Workspace ─────────────────────────────────
        chunks.Add(MakeChunk("shell-sync-workspace", "Cloud Sync & Multi-Workspace",
            """
            Cloud Sync: Bi-directional workspace synchronization.
            - Syncs all plugin data to a private cloud relay
            - Conflict resolution for concurrent edits
            - Encryption in transit and at rest
            - Configure sync in Settings

            Multi-Workspace: Separate data directories per workspace.
            - Each workspace has independent data, settings, and plugin activation
            - Switch workspaces from the user menu (avatar in top-left)
            - Create, rename, or delete workspaces

            Backup & Recovery:
            - Create workspace snapshots from Settings
            - Restore from any previous snapshot
            - Automatic backup scheduling available
            """));

        // ── Theme & Appearance ───────────────────────────────────────────
        chunks.Add(MakeChunk("shell-theme", "Theme & Appearance",
            """
            Theme Management in PrivStack:
            - Light and dark themes available, toggled from Settings or the user menu
            - Custom color palette editor for personalizing the accent color
            - Fluent design system with consistent styling across all plugins
            - Sidebar can be collapsed (Cmd+\) for more content space
            """));

        // ── Speech & Audio ───────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-speech", "Speech-to-Text",
            """
            Speech-to-Text (Cmd+M): Voice input using Whisper STT.
            - Press Cmd+M / Ctrl+M to start recording from any text input
            - Transcription is inserted at the cursor position in the active text field, code editor, or rich text editor
            - Works offline using the local Whisper model
            - Recording indicator shows in the toolbar while active
            """));

        // ── Reminders & Notifications ────────────────────────────────────
        chunks.Add(MakeChunk("shell-reminders", "Reminders & Notifications",
            """
            Reminder System: Aggregates reminders from all plugins implementing IReminderProvider.
            - Tasks with due dates generate reminders
            - Calendar events with reminder settings
            - Habit tracking reminders
            - Financial bill/payment reminders
            - System tray notifications on the desktop
            """));

        // ── Security & Vault ─────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-security", "Security & Master Password",
            """
            Security features in PrivStack:
            - Master Password: Protects the encrypted vault for sensitive data
            - Vault stores OAuth tokens, API keys, and encrypted file attachments
            - Plugins access the vault through IVaultConsumer capability interface
            - Emergency Kit: Generate a PDF with recovery information (from Settings)
            - All data stored locally on the user's device — no data leaves the machine unless Cloud Sync is explicitly enabled
            """));

        // ── Plugin Ecosystem Overview ────────────────────────────────────
        chunks.Add(MakeChunk("shell-plugins", "Plugin Ecosystem Overview",
            """
            PrivStack Plugin Ecosystem: 14 built-in plugins covering personal productivity.

            Available Plugins:
            - Notes (privstack.notes): Block editor with wiki links, page hierarchy, sticky notes, templates, GitHub wiki sync
            - Tasks (privstack.tasks): Task management with list/kanban/timeline/calendar views, time tracking, GitHub Issues sync
            - Calendar (privstack.calendar): Event management with month/week/day/agenda views, recurrence, CalDAV/ICS sync
            - Contacts (privstack.contacts): CRM with contacts, companies, groups, interactions, org charts, import (vCard/CSV)
            - Journal (privstack.journal): Daily journaling with templates, mood tracking, streaks, calendar heatmap, insights
            - Finance (privstack.finance): Double-entry bookkeeping with ZBB/envelope budgeting, reconciliation, reports
            - Habits (privstack.habits): Habit tracking with streaks and goal management with milestones and analytics
            - Mail (privstack.email): IMAP/SMTP email client with multi-account, OAuth, threading, reader mode
            - RSS (privstack.rss): RSS/Atom feed reader with reader mode, OPML import/export
            - Snippets (privstack.snippets): Code snippet manager with syntax highlighting (23 languages), collections
            - Data (privstack.data): Tabular dataset browser with SQL query engine, CSV/Notion import
            - Files (privstack.files): Encrypted file vault and media library with folder organization
            - WebClips (privstack.webclips): Web clipping, read-it-later queue, browsing history, transcription
            - Canvas (privstack.canvas): Infinite canvas whiteboards with cross-plugin entity references

            All plugins are isolated and communicate through the SDK. Each plugin can be individually enabled or disabled per workspace.
            """));

        return Task.FromResult(new IndexableContentResult { Chunks = chunks });
    }

    private static ContentChunk MakeChunk(string id, string title, string text)
    {
        text = text.Trim();
        return new ContentChunk
        {
            EntityId = id,
            EntityType = "shell_context",
            PluginId = ShellPluginId,
            ChunkPath = "content",
            Text = text,
            ContentHash = ComputeHash(text),
            Title = title,
            LinkType = "shell_context",
            ModifiedAt = DateTimeOffset.UtcNow,
        };
    }

    private static string ComputeHash(string text)
    {
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(text));
        return Convert.ToHexString(bytes);
    }
}
