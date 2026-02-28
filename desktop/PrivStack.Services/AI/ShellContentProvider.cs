using System.Security.Cryptography;
using System.Text;
using PrivStack.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using Serilog;

namespace PrivStack.Services.AI;

/// <summary>
/// Shell-level RAG content provider that indexes global features, shortcuts,
/// and capabilities so the AI can answer questions about app-wide functionality.
/// Also dynamically indexes intent descriptors and quick actions so the AI can
/// discover available actions and shortcuts via semantic search.
/// Registered with the CapabilityBroker (not a plugin) so RagIndexService discovers it.
/// </summary>
internal sealed class ShellContentProvider : IIndexableContentProvider
{
    private static readonly ILogger _log = Log.ForContext<ShellContentProvider>();
    private const string ShellPluginId = "privstack.desktop";

    private readonly IIntentEngine _intentEngine;
    private readonly IPluginRegistry _pluginRegistry;

    public ShellContentProvider(IIntentEngine intentEngine, IPluginRegistry pluginRegistry)
    {
        _intentEngine = intentEngine;
        _pluginRegistry = pluginRegistry;
    }

    public Task<IndexableContentResult> GetIndexableContentAsync(
        IndexableContentRequest request, CancellationToken ct = default)
    {
        var chunks = new List<ContentChunk>();

        // ── Intent Action Chunks (one per intent) ─────────────────────────
        IndexIntentActions(chunks);

        // ── Quick Action Chunks (one per quick action from plugins) ──────
        IndexQuickActions(chunks);

        // ── Global Keyboard Shortcuts ────────────────────────────────────
        chunks.Add(MakeChunk("shell-shortcuts", "Global Keyboard Shortcuts",
            BuildShortcutsChunkText()));

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

            How to interact with Duncan:
            - Click the star icon in the top-right corner of the app to open the AI tray
            - Type a message in the "Ask Duncan..." text box and press Enter to chat
            - Duncan can answer questions about your data, summarize pages, brainstorm ideas, and explain features
            - Duncan has access to your workspace data via RAG (semantic search) — it searches your notes, tasks, journal entries, contacts, and other entities to provide contextual answers
            - Duncan remembers context within a conversation. Start a new chat to reset context
            - When Duncan has proactive insights or intent suggestions, a notification balloon appears on the star icon

            Features:
            - Free-form chat: Ask questions, get summaries, brainstorm, draft content
            - Intent suggestions: Duncan detects actionable signals and suggests plugin actions (see "Intent System" for details)
            - Content suggestions: Plugins push rich suggestion cards (e.g., AI-rewritten task descriptions) into the tray
            - RAG search: Duncan searches your entire knowledge base using 768-dimensional semantic embeddings to find relevant context before answering
            - Conversation history: Past conversations are saved and can be resumed from the history panel
            - AI memory: Duncan learns your preferences across conversations (e.g., preferred formats, common topics)

            AI Provider Support (configure in Settings):
            - OpenAI (GPT-4o, GPT-4, GPT-3.5-turbo) — requires API key
            - Anthropic (Claude 3.5 Sonnet, Claude 3 Opus/Haiku) — requires API key
            - Google Gemini (Gemini Pro, Gemini Flash) — requires API key
            - Local LLaMA models — runs entirely on-device, fully offline, with token streaming. Download models from Settings. Best for privacy-sensitive use cases

            Configuration: Go to Settings (gear icon) > AI to configure API keys, select the active provider, adjust response length preferences, or download a local model.

            Response length: Duncan automatically classifies your message as short/medium/long and adjusts response length accordingly. Short questions get 1-2 sentence answers, detailed requests get thorough multi-paragraph responses.
            """));

        // ── Intent System ──────────────────────────────────────────────────
        chunks.Add(MakeChunk("shell-intents", "Intent System — AI-Powered Actions",
            """
            Intent System in PrivStack:
            Intents are AI-powered actions that let Duncan create, query, and manage data across all plugins using natural language. Each plugin declares the intents it supports, and Duncan matches your requests to the right intent automatically.

            How intents work:
            1. You type a natural language request in the AI tray (e.g., "Create a task to review the budget report by Friday")
            2. Duncan's Intent Engine classifies your message against all registered intents across all plugins
            3. If a match is found, Duncan extracts the relevant parameters (called "slots") from your message
            4. The intent is executed by the owning plugin, creating or querying the actual data
            5. Duncan confirms the action and can provide a link to the created entity

            Intent signals: Duncan also proactively monitors signals from plugins (e.g., you're reading an email about a meeting) and suggests relevant intents in the AI tray (e.g., "Create a calendar event for this meeting?"). These appear as suggestion cards you can approve or dismiss.

            Available intents by plugin:

            Notes: notes.create_note (create a new note page with title and content)

            Tasks: tasks.create_task (create a task with title, description, priority, due date, tags)

            Calendar: calendar.create_event (create an event with title, date, time, duration, location, description)

            Contacts: contacts.create_contact (create a contact with name, email, phone, company, notes)

            Journal: journal.create_entry (create a journal entry with title, content, mood, tags)

            Finance: finance.create_transaction (create transaction with payee, amount, account, date, memo), finance.check_balance (check account balance), finance.check_budget (check category budget status), finance.transfer_between_categories (move budget money between categories), finance.get_monthly_summary (monthly income/expense summary), finance.get_spending_breakdown (spending by category for date range), finance.get_budget_health (budget health check), finance.get_financial_trends (trends over time), finance.get_account_overview (all account balances), finance.suggest_from_receipt (suggest transaction from receipt text)

            Habits: habits.log_habit (log a habit completion), habits.create_habit (create a new habit), habits.create_goal (create a goal with milestones)

            Email: email.draft_email (draft an email with to, subject, body)

            Snippets: snippets.save_snippet (save a code snippet with title, language, code, collection)

            RSS: rss.add_feed (subscribe to an RSS feed by URL)

            Files: (navigate to files via deep links)

            WebClips: (navigate to clips via deep links)

            Examples of what you can ask Duncan:
            - "Add a task to review the Q4 budget, due next Monday, high priority"
            - "How much did I spend on groceries last month?"
            - "Create a meeting event for Thursday at 2pm called Sprint Planning"
            - "Draft an email to john@example.com about the project update"
            - "What's my checking account balance?"
            - "Log my meditation habit for today"
            - "Save this Python snippet: def hello(): print('world')"
            - "What's my budget health this month?"
            - "Create a contact for Jane Smith at Acme Corp, jane@acme.com"
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

    private void IndexIntentActions(List<ContentChunk> chunks)
    {
        var intents = _intentEngine.GetAllAvailableIntents();
        foreach (var intent in intents)
        {
            var slotLines = string.Join("\n", intent.Slots.Select(s =>
                $"  - {s.Name} ({s.Type}){(s.Required ? " [REQUIRED]" : "")}: {s.Description}"));

            var requiredSlotExample = string.Join(", ",
                intent.Slots.Where(s => s.Required).Select(s => $"\"{s.Name}\": \"value\""));
            var actionExample = "{\"intent_id\": \"" + intent.IntentId + "\", \"slots\": {" + requiredSlotExample + "}}";

            var text = $"""
                ACTION: {intent.DisplayName}
                Intent ID: {intent.IntentId}
                Plugin: {intent.PluginId}
                Description: {intent.Description}

                Slots:
                {slotLines}

                To execute this action, include an [ACTION] block in your response:
                [ACTION]
                {actionExample}
                [/ACTION]
                """.Trim();

            chunks.Add(new ContentChunk
            {
                EntityId = $"intent-{intent.IntentId}",
                EntityType = "intent_action",
                PluginId = ShellPluginId,
                ChunkPath = "content",
                Text = text,
                ContentHash = ComputeHash(text),
                Title = $"Action: {intent.DisplayName}",
                LinkType = "intent_action",
                ModifiedAt = DateTimeOffset.UtcNow,
            });
        }
    }

    private void IndexQuickActions(List<ContentChunk> chunks)
    {
        var providers = _pluginRegistry.GetCapabilityProviders<IQuickActionProvider>();
        foreach (var provider in providers)
        {
            IReadOnlyList<QuickActionDescriptor> actions;
            try { actions = provider.GetQuickActions(); }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to get quick actions from provider");
                continue;
            }

            foreach (var action in actions)
            {
                var shortcutLine = !string.IsNullOrEmpty(action.DefaultShortcutHint)
                    ? $"\nKeyboard Shortcut: {action.DefaultShortcutHint}"
                    : "";

                var text = $"""
                    Quick Action: {action.DisplayName}
                    Plugin: {action.PluginId}
                    Description: {action.Description ?? action.DisplayName}{shortcutLine}
                    Category: {action.Category}
                    Has UI overlay: {(action.HasUI ? "Yes (opens a modal form)" : "No (executes immediately)")}

                    To use this quick action, press {action.DefaultShortcutHint ?? "the keyboard shortcut shown in the command palette (Cmd+K)"}, or open the Command Palette (Cmd+K) and search for "{action.DisplayName}".
                    """.Trim();

                chunks.Add(new ContentChunk
                {
                    EntityId = $"quick-action-{action.ActionId}",
                    EntityType = "shell_context",
                    PluginId = ShellPluginId,
                    ChunkPath = "content",
                    Text = text,
                    ContentHash = ComputeHash(text),
                    Title = $"Quick Action: {action.DisplayName}",
                    LinkType = "shell_context",
                    ModifiedAt = DateTimeOffset.UtcNow,
                });
            }
        }
    }

    /// <summary>
    /// Builds the keyboard shortcuts chunk text with shell-level shortcuts (hardcoded)
    /// and plugin quick action shortcuts (dynamically discovered).
    /// </summary>
    private string BuildShortcutsChunkText()
    {
        var sb = new StringBuilder();
        sb.AppendLine("PrivStack keyboard shortcuts and navigation:");
        sb.AppendLine();
        sb.AppendLine("Shell-level shortcuts (always available):");
        sb.AppendLine("- Cmd+K / Ctrl+K: Open Universal Search (search across all plugins, navigate anywhere)");
        sb.AppendLine("- Cmd+I / Ctrl+I: Toggle the Info Panel (right sidebar showing entity details and backlinks)");
        sb.AppendLine("- Cmd+M / Ctrl+M: Toggle Speech Recording (Whisper speech-to-text, inserts at cursor)");
        sb.AppendLine("- Cmd+\\ / Ctrl+\\: Toggle sidebar collapsed/expanded");
        sb.AppendLine("- Cmd+1 through Cmd+9 / Ctrl+1-9: Switch to plugin tab by position (1=first plugin, 2=second, etc.)");
        sb.AppendLine("- Escape: Close active overlay, panel, or modal (AI tray, info panel, quick action, search dropdown)");
        sb.AppendLine("- Cmd+Shift+N: New Page from Template (Notes)");

        // Dynamically append plugin quick action shortcuts
        var providers = _pluginRegistry.GetCapabilityProviders<IQuickActionProvider>();
        var quickActions = new List<(string Shortcut, string Name, string Plugin)>();
        foreach (var provider in providers)
        {
            try
            {
                foreach (var action in provider.GetQuickActions())
                {
                    if (!string.IsNullOrEmpty(action.DefaultShortcutHint))
                        quickActions.Add((action.DefaultShortcutHint, action.DisplayName, action.PluginId));
                }
            }
            catch { /* provider failed, skip */ }
        }

        if (quickActions.Count > 0)
        {
            sb.AppendLine();
            sb.AppendLine("Plugin quick action shortcuts:");
            foreach (var (shortcut, name, plugin) in quickActions)
                sb.AppendLine($"- {shortcut}: {name} ({plugin})");
        }

        return sb.ToString().TrimEnd();
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
