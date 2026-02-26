using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Services.AI;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Helpers;
using PrivStack.Sdk.Services;
using Serilog;

namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Free-form chat input logic with conversation persistence and token tracking.
/// </summary>
public partial class AiSuggestionTrayViewModel
{
    private const int MaxHistoryMessages = 20;
    private const double TokenWarningThreshold = 0.8;
    private const int MaxWikiLinkResolutions = 5;

    public string ChatWatermark { get; } = $"Ask {AiPersona.Name}...";

    [ObservableProperty]
    private string? _chatInputText;

    [ObservableProperty]
    private bool _isSendingChat;

    [ObservableProperty]
    private string? _activeSessionId;

    [ObservableProperty]
    private int _estimatedTokensUsed;

    [ObservableProperty]
    private int _contextWindowSize;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(TokenWarningText))]
    private bool _isNearTokenLimit;

    public string TokenWarningText => "Context is getting long. Start a new chat for best results.";

    // ── Conversational Entity Tracking ──────────────────────────────

    private string? _lastActionEntityId;
    private string? _lastActionEntityType;

    [RelayCommand]
    private void StartNewChat()
    {
        // Current session is already persisted via AddMessage calls
        ActiveSessionId = null;
        EstimatedTokensUsed = 0;
        IsNearTokenLimit = false;
        _lastActionEntityId = null;
        _lastActionEntityType = null;
        ChatMessages.Clear();
        UpdateCounts();
        RefreshConversationHistory();
    }

    [RelayCommand(CanExecute = nameof(CanSendChat))]
    private async Task SendChatMessageAsync()
    {
        var text = ChatInputText?.Trim();
        if (string.IsNullOrEmpty(text)) return;

        ChatInputText = null;
        IsSendingChat = true;

        // Ensure we have an active session
        if (ActiveSessionId == null)
        {
            var session = _conversationStore.CreateSession();
            ActiveSessionId = session.Id;
            RefreshContextWindowSize();
        }

        // Persist user message
        _conversationStore.AddMessage(ActiveSessionId, "user", text);

        // User bubble
        var userMsg = new AiChatMessageViewModel(ChatMessageRole.User) { UserLabel = text };
        ChatMessages.Add(userMsg);

        // Assistant bubble (loading)
        var assistantMsg = new AiChatMessageViewModel(ChatMessageRole.Assistant)
        {
            State = ChatMessageState.Loading
        };
        ChatMessages.Add(assistantMsg);
        UpdateCounts();
        RequestScrollToBottom();

        try
        {
            // Capture UI-thread state before jumping to background
            var userName = _appSettings.Settings.UserDisplayName
                ?? Environment.UserName ?? "there";
            var tier = AiPersona.Classify(text);
            var isCloud = !IsActiveProviderLocal();
            var activePluginCtx = _activePluginContext;
            var activeItemCtxFull = _activeItemContextFull;
            var activeItemCtxShort = _activeItemContextShort;

            // Capture conversational entity tracking state
            var lastEntityId = _lastActionEntityId;
            var lastEntityType = _lastActionEntityType;

            // Snapshot chat messages on UI thread (ObservableCollection is not thread-safe)
            var chatSnapshot = ChatMessages
                .Where(m => m.SuggestionId == null
                    && !(m.Role == ChatMessageRole.Assistant && m.State == ChatMessageState.Loading))
                .TakeLast(MaxHistoryMessages)
                .Select(m => new AiChatMessage
                {
                    Role = m.Role == ChatMessageRole.User ? "user" : "assistant",
                    Content = m.Role == ChatMessageRole.User ? m.UserLabel ?? "" : m.Content ?? ""
                })
                .ToList();

            // Run all heavy work (RAG, wiki-link resolution, prompt building, AI call) off the UI thread
            var (response, request) = await Task.Run(async () =>
            {
                var (ragContext, hasIntentActions) = await BuildRagContextWithIntentsAsync(text);

                // Resolve wiki-links in background (I/O bound — was blocking UI thread)
                var resolvedEntityContext = await ResolveWikiLinksAsync(text);

                // Build conversation history from snapshot (exclude last user message — it's the current prompt)
                IReadOnlyList<AiChatMessage>? conversationHistory = null;
                if (chatSnapshot.Count > 0)
                {
                    if (chatSnapshot[^1].Role == "user")
                        chatSnapshot.RemoveAt(chatSnapshot.Count - 1);
                    if (chatSnapshot.Count > 0)
                        conversationHistory = chatSnapshot;
                }

                AiRequest req;
                if (isCloud)
                {
                    var memoryContext = _memoryService.FormatForPrompt();
                    var systemPrompt = AiPersona.GetCloudSystemPrompt(tier, userName, memoryContext);

                    if (!string.IsNullOrEmpty(activePluginCtx))
                        systemPrompt += $"\n\n{activePluginCtx}";

                    if (!string.IsNullOrEmpty(ragContext))
                        systemPrompt += $"\n\n{ragContext}";

                    if (!string.IsNullOrEmpty(activeItemCtxFull))
                        systemPrompt += $"\n\n{activeItemCtxFull}";

                    if (!string.IsNullOrEmpty(resolvedEntityContext))
                        systemPrompt += $"\n\n{resolvedEntityContext}";

                    if (!string.IsNullOrEmpty(lastEntityId))
                        systemPrompt += $"\n\nIn this conversation, you most recently acted on: {lastEntityType} ID \"{lastEntityId}\". If the user says \"that task\", \"it\", or similar, use this ID as task_id.";

                    // If RAG found intent_action chunks, inject the ACTION format header
                    if (hasIntentActions)
                        systemPrompt += $"\n\n{ActionFormatHeader}";

                    req = new AiRequest
                    {
                        SystemPrompt = systemPrompt,
                        UserPrompt = text,
                        MaxTokens = AiPersona.CloudMaxTokensFor(tier),
                        Temperature = 0.4,
                        FeatureId = "tray.chat",
                        ConversationHistory = conversationHistory
                    };
                }
                else
                {
                    var systemPrompt = AiPersona.GetSystemPrompt(tier, userName);

                    if (!string.IsNullOrEmpty(activePluginCtx))
                        systemPrompt += $"\n\n{activePluginCtx}";

                    if (!string.IsNullOrEmpty(ragContext))
                        systemPrompt += $"\n\n{ragContext}";

                    if (!string.IsNullOrEmpty(activeItemCtxShort))
                        systemPrompt += $"\n\n{activeItemCtxShort}";

                    if (!string.IsNullOrEmpty(resolvedEntityContext))
                        systemPrompt += $"\n\n{resolvedEntityContext}";

                    if (!string.IsNullOrEmpty(lastEntityId))
                        systemPrompt += $"\n\nIn this conversation, you most recently acted on: {lastEntityType} ID \"{lastEntityId}\". If the user says \"that task\", \"it\", or similar, use this ID as task_id.";

                    // If RAG found intent_action chunks, inject the ACTION format header
                    if (hasIntentActions)
                        systemPrompt += $"\n\n{ActionFormatHeader}";

                    req = new AiRequest
                    {
                        SystemPrompt = systemPrompt,
                        UserPrompt = text,
                        MaxTokens = AiPersona.MaxTokensFor(tier),
                        Temperature = 0.4,
                        FeatureId = "tray.chat",
                        ConversationHistory = conversationHistory
                    };
                }

                AiResponse resp;
                if (!isCloud)
                {
                    _dispatcher.Post(() => assistantMsg.State = ChatMessageState.Streaming);
                    resp = await _aiService.StreamCompleteAsync(req, partialContent =>
                    {
                        _dispatcher.Post(() =>
                        {
                            assistantMsg.Content = AiPersona.Sanitize(partialContent, tier);
                        });
                    });
                }
                else
                {
                    resp = await _aiService.CompleteAsync(req);
                }

                return (resp, req);
            });

            // Back on UI thread — process the response
            if (response.Success && !string.IsNullOrEmpty(response.Content))
            {
                // Log raw response for debugging action block issues
                _log.Debug("Raw AI response ({Length} chars): {Content}",
                    response.Content.Length, response.Content);

                // Parse action blocks before sanitization (sanitize strips them)
                var (cleanContent, actions) = ParseActionBlocks(response.Content);
                _log.Debug("Chat response: {ActionCount} action blocks parsed from response ({ResponseLength} chars)",
                    actions.Count, response.Content.Length);
                if (actions.Count == 0 && response.Content.Contains("[ACTION]", StringComparison.OrdinalIgnoreCase))
                    _log.Warning("Response contained [ACTION] text but parsing found 0 blocks — possible format issue");
                var content = AiPersona.Sanitize(cleanContent, tier);

                assistantMsg.Content = content;
                assistantMsg.State = ChatMessageState.Ready;

                // Persist assistant message
                _conversationStore.AddMessage(ActiveSessionId!, "assistant", content);

                // Execute parsed intent actions and collect results for conversation feedback
                var actionResults = new List<string>();
                foreach (var action in actions)
                {
                    try
                    {
                        var result = await _intentEngine.ExecuteDirectAsync(
                            action.IntentId, action.Slots);

                        // Build display text including warnings
                        var displayText = result.Success
                            ? $"\u2713 {result.Summary ?? "Done!"}"
                            : $"\u2717 {result.ErrorMessage ?? "Action failed."}";

                        if (result.Warnings is { Count: > 0 })
                        {
                            var warningText = string.Join("; ", result.Warnings);
                            displayText += $"\n\u26a0 {warningText}";
                        }

                        var confirmMsg = new AiChatMessageViewModel(ChatMessageRole.Assistant)
                        {
                            Content = displayText,
                            State = result.Success
                                ? ChatMessageState.Applied
                                : ChatMessageState.Error,
                        };
                        ChatMessages.Add(confirmMsg);

                        // Collect for conversation history injection
                        actionResults.Add(displayText);

                        // Track the last successfully acted-on entity for conversational context
                        if (result.Success && !string.IsNullOrEmpty(result.CreatedEntityId))
                        {
                            _lastActionEntityId = result.CreatedEntityId;
                            _lastActionEntityType = result.CreatedEntityType;
                        }
                    }
                    catch (Exception ex)
                    {
                        _log.Warning(ex, "Failed to execute chat action: {IntentId}", action.IntentId);
                        var errorText = $"\u2717 Failed to execute action: {ex.Message}";
                        ChatMessages.Add(new AiChatMessageViewModel(ChatMessageRole.Assistant)
                        {
                            Content = errorText,
                            State = ChatMessageState.Error,
                        });
                        actionResults.Add(errorText);
                    }
                }

                // Inject action results into conversation history so the AI
                // knows what succeeded/failed on the next turn
                if (actionResults.Count > 0)
                {
                    var resultsSummary = "[Action Results]\n" +
                        string.Join("\n", actionResults);
                    _conversationStore.AddMessage(ActiveSessionId!, "assistant", resultsSummary);
                }

                // Fire-and-forget memory extraction for cloud responses
                if (isCloud)
                    _ = _memoryExtractor.EvaluateAsync(text, content);
            }
            else
            {
                assistantMsg.ErrorMessage = response.ErrorMessage ?? "AI request failed";
                assistantMsg.State = ChatMessageState.Error;
            }
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Free-form chat request failed");
            assistantMsg.ErrorMessage = $"Error: {ex.Message}";
            assistantMsg.State = ChatMessageState.Error;
        }
        finally
        {
            IsSendingChat = false;
            UpdateTokenEstimate();
            RequestScrollToBottom();
        }
    }

    private bool CanSendChat() => !string.IsNullOrWhiteSpace(ChatInputText) && !IsSendingChat;

    /// <summary>
    /// Entry point for Whisper voice commands when no text input is focused.
    /// Sets ChatInputText and triggers the standard send pipeline.
    /// </summary>
    public async Task SendVoiceMessageAsync(string text)
    {
        if (string.IsNullOrWhiteSpace(text) || IsSendingChat) return;
        ChatInputText = text;
        await SendChatMessageAsync();
    }

    partial void OnChatInputTextChanged(string? value) => SendChatMessageCommand.NotifyCanExecuteChanged();
    partial void OnIsSendingChatChanged(bool value) => SendChatMessageCommand.NotifyCanExecuteChanged();

    private void UpdateTokenEstimate()
    {
        if (ActiveSessionId == null) return;
        var session = _conversationStore.GetSession(ActiveSessionId);
        if (session == null) return;

        EstimatedTokensUsed = session.EstimatedTokens;
        IsNearTokenLimit = ContextWindowSize > 0
            && EstimatedTokensUsed > ContextWindowSize * TokenWarningThreshold;
    }

    private void RefreshContextWindowSize()
    {
        var modelInfo = _aiService.GetActiveModelInfo();
        ContextWindowSize = modelInfo?.ContextWindowTokens ?? 0;
    }

    private bool IsActiveProviderLocal()
    {
        var providerId = _appSettings.Settings.AiProvider;
        if (string.IsNullOrEmpty(providerId) || providerId == "none")
            return true;

        var providers = _aiService.GetProviders();
        var active = providers.FirstOrDefault(p => p.Id == providerId);
        return active?.IsLocal ?? true;
    }

    // ── Wiki-Link Resolution ────────────────────────────────────────

    /// <summary>
    /// Extracts wiki-links from the user message and resolves them to entity metadata
    /// via ILinkableItemProvider. Returns a context string for system prompt injection.
    /// </summary>
    private async Task<string?> ResolveWikiLinksAsync(string text)
    {
        try
        {
            var links = WikiLinkParser.ParseLinks(text);
            if (links.Count == 0) return null;

            var providers = _pluginRegistry.GetCapabilityProviders<ILinkableItemProvider>();
            if (providers.Count == 0) return null;

            var sb = new StringBuilder();
            sb.AppendLine("The user referenced the following entities (use their IDs in ACTION blocks):");

            var resolved = 0;
            foreach (var link in links)
            {
                if (resolved >= MaxWikiLinkResolutions) break;

                var provider = providers.FirstOrDefault(p =>
                    p.LinkType.Equals(link.LinkType, StringComparison.OrdinalIgnoreCase));
                if (provider is null) continue;

                var item = await provider.GetItemByIdAsync(link.EntityId);
                if (item is null) continue;

                sb.AppendLine($"- [[{item.LinkType}:{item.Id}|{item.Title}]] — Type: {item.LinkType}, ID: {item.Id}, Title: {item.Title}" +
                    (item.Subtitle is not null ? $", Details: {item.Subtitle}" : ""));
                resolved++;
            }

            return resolved > 0 ? sb.ToString().TrimEnd() : null;
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to resolve wiki-links in chat message");
            return null;
        }
    }

    // ── ACTION Format Header (injected only when RAG finds intent chunks) ──

    private const string ActionFormatHeader = """
        CRITICAL: You can perform REAL actions using [ACTION] blocks. Without an [ACTION] block, NOTHING happens — the system ignores your words completely.
        ABSOLUTE RULE: NEVER write "✓ Created", "Done", "I've created", or ANY claim of completion UNLESS you include the corresponding [ACTION] block. Writing checkmarks or success messages without [ACTION] blocks is LYING to the user — nothing actually happened.
        If the user asks you to do 6 things, you MUST include 6 separate [ACTION] blocks — one per action. There are no shortcuts.
        Place ALL [ACTION] blocks at the END of your response, after your conversational message.
        Format: [ACTION]{"intent_id": "exact.id", "slots": {...}}[/ACTION]
        IMPORTANT: You MUST use the EXACT intent_id values shown in the ACTION descriptions above (e.g. "tasks.update_task", NOT "tasks.update").
        Do NOT abbreviate, shorten, or invent intent IDs. Copy them exactly from the action descriptions.
        SLOT NAMES: You MUST ONLY use slot names that are explicitly listed in the action descriptions above. NEVER invent slot names like "parent_task_id", "parent_id", "subtask", or any name not in the description. Unknown slots are silently stripped and will have NO effect. If a capability doesn't exist (e.g. nesting tasks), say so — do not guess at slot names.
        To LINK existing entities, use the tasks.add_link intent with task_id, target_id, target_link_type, and relationship. Do NOT re-create entities when linking.
        You CAN use actions from ANY plugin, not just the one the user is currently viewing.
        For example, if the user asks to create a note while viewing Finance, use the notes.create_note action.
        If no relevant action exists in the descriptions above, say you can't do that yet — do NOT pretend you did it.
        Slot values must be strings or arrays of strings. For list-type slots like add_checklist or tags, you may use a JSON array: "add_checklist": ["item 1", "item 2"].
        Use the slot name "add_checklist" (not "checklist") when adding checklist items to a task.
        After executing actions, you will see [Action Results] showing what succeeded or failed. Use this feedback to correct errors on subsequent turns.
        """;

    /// <summary>
    /// Runs semantic search against the RAG vector index and formats matching results.
    /// Performs TWO searches: one for general content, one specifically for intent_action
    /// chunks (which tend to get crowded out by entity data in the general search).
    /// Returns the formatted context string and whether any intent_action chunks were found
    /// (which triggers ACTION format header injection in the system prompt).
    /// </summary>
    private async Task<(string? Context, bool HasIntentActions)> BuildRagContextWithIntentsAsync(string query)
    {
        if (!_ragSearchService.IsReady)
            return (null, false);

        try
        {
            var isCloud = !IsActiveProviderLocal();
            var dataLimit = isCloud ? 8 : 6;
            var intentLimit = isCloud ? 5 : 3;
            var maxChunkChars = isCloud ? 500 : 800;

            // Fetch more candidates than needed so we can apply plugin diversity.
            // This ensures cross-plugin queries (e.g. asking about Notes from Tasks)
            // still surface relevant results from other plugins.
            var fetchLimit = dataLimit * 3;

            // Two parallel searches: general data + intent actions specifically
            var dataTask = _ragSearchService.SearchAsync(query, fetchLimit);
            var intentTask = _ragSearchService.SearchAsync(
                query, intentLimit, ["intent_action"]);

            await Task.WhenAll(dataTask, intentTask);

            var scoredResults = dataTask.Result.Where(r => r.Score >= 0.3).ToList();
            var intentResults = intentTask.Result.Where(r => r.Score >= 0.25).ToList();

            // Apply plugin diversity: ensure results from multiple plugins are
            // represented. If the query mentions a specific plugin, boost it.
            var dataResults = ApplyPluginDiversity(scoredResults, dataLimit, query);

            // Deduplicate: remove intent_action results already in general results
            var dataEntityIds = new HashSet<string>(dataResults.Select(r => r.EntityId));
            var uniqueIntentResults = intentResults
                .Where(r => !dataEntityIds.Contains(r.EntityId))
                .ToList();

            // Merge: data results first, then additional intent results
            var allResults = dataResults.Concat(uniqueIntentResults).ToList();

            if (allResults.Count == 0)
                return (null, false);

            var hasIntentActions = allResults.Any(r => r.EntityType == "intent_action");
            _log.Debug("RAG context: {DataCount} data + {IntentCount} intent action results ({TotalIntents} intent actions total) injected into system prompt",
                dataResults.Count, uniqueIntentResults.Count,
                allResults.Count(r => r.EntityType == "intent_action"));

            var sb = new StringBuilder();
            sb.AppendLine("Relevant information from the user's data:");
            sb.AppendLine();

            foreach (var result in allResults)
            {
                sb.AppendLine($"[{result.EntityType}] {result.Title} (score: {result.Score:F2})");

                if (!string.IsNullOrWhiteSpace(result.ChunkText))
                {
                    var text = result.ChunkText.Length > maxChunkChars
                        ? result.ChunkText[..maxChunkChars] + "..."
                        : result.ChunkText;
                    sb.AppendLine(text);
                }

                sb.AppendLine();
            }

            return (sb.ToString().TrimEnd(), hasIntentActions);
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "RAG search failed during chat, continuing without context");
            return (null, false);
        }
    }

    // ── Plugin Diversity ─────────────────────────────────────────────

    /// <summary>
    /// Maps user-facing plugin names/keywords to plugin IDs for query-based boosting.
    /// </summary>
    private static readonly Dictionary<string, string> PluginKeywords = new(StringComparer.OrdinalIgnoreCase)
    {
        ["note"] = "notes", ["notes"] = "notes", ["page"] = "notes", ["pages"] = "notes", ["sticky"] = "notes",
        ["task"] = "tasks", ["tasks"] = "tasks", ["todo"] = "tasks", ["kanban"] = "tasks",
        ["calendar"] = "calendar", ["event"] = "calendar", ["events"] = "calendar", ["schedule"] = "calendar",
        ["contact"] = "contacts", ["contacts"] = "contacts", ["people"] = "contacts", ["person"] = "contacts",
        ["journal"] = "journal", ["diary"] = "journal",
        ["finance"] = "finance", ["budget"] = "finance", ["transaction"] = "finance", ["account"] = "finance",
        ["file"] = "files", ["files"] = "files",
        ["snippet"] = "snippets", ["snippets"] = "snippets", ["code"] = "snippets",
        ["rss"] = "rss", ["feed"] = "rss", ["feeds"] = "rss",
        ["email"] = "email", ["mail"] = "email",
        ["habit"] = "habits", ["habits"] = "habits",
        ["clip"] = "webclips", ["webclip"] = "webclips",
    };

    /// <summary>
    /// Ensures RAG results from multiple plugins are represented in the final set.
    /// If the query mentions a specific plugin by name, results from that plugin
    /// are prioritized (half the slots reserved). Otherwise, round-robin interleaves
    /// by plugin so no single one dominates.
    /// </summary>
    private static List<RagSearchResult> ApplyPluginDiversity(
        List<RagSearchResult> candidates, int limit, string query)
    {
        if (candidates.Count <= limit)
            return candidates;

        // Detect if the user is asking about a specific plugin
        string? boostedPluginId = null;
        var queryWords = query.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        foreach (var word in queryWords)
        {
            // Strip punctuation for matching
            var clean = word.TrimEnd('?', '.', ',', '!', ':', ';');
            if (PluginKeywords.TryGetValue(clean, out var pluginId))
            {
                boostedPluginId = pluginId;
                break;
            }
        }

        var result = new List<RagSearchResult>(limit);

        if (boostedPluginId != null)
        {
            // Reserve half the slots for the mentioned plugin
            var boostedSlots = limit / 2;
            var otherSlots = limit - boostedSlots;

            var boosted = candidates
                .Where(r => r.PluginId.Equals(boostedPluginId, StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(r => r.Score)
                .Take(boostedSlots)
                .ToList();

            var others = candidates
                .Where(r => !r.PluginId.Equals(boostedPluginId, StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(r => r.Score)
                .Take(otherSlots)
                .ToList();

            result.AddRange(boosted);
            result.AddRange(others);

            // If boosted didn't fill its slots, backfill from others
            if (result.Count < limit)
            {
                var existing = new HashSet<string>(result.Select(r => r.EntityId));
                var backfill = candidates
                    .Where(r => !existing.Contains(r.EntityId))
                    .OrderByDescending(r => r.Score)
                    .Take(limit - result.Count);
                result.AddRange(backfill);
            }
        }
        else
        {
            // No specific plugin mentioned — round-robin interleave
            var groups = candidates
                .GroupBy(r => r.PluginId)
                .Select(g => new Queue<RagSearchResult>(g.OrderByDescending(r => r.Score)))
                .OrderByDescending(q => q.Peek().Score)
                .ToList();

            while (result.Count < limit && groups.Count > 0)
            {
                for (var i = groups.Count - 1; i >= 0; i--)
                {
                    if (result.Count >= limit) break;
                    result.Add(groups[i].Dequeue());
                    if (groups[i].Count == 0)
                        groups.RemoveAt(i);
                }
            }
        }

        // Re-sort by score so highest relevance appears first in the prompt
        result.Sort((a, b) => b.Score.CompareTo(a.Score));
        return result;
    }

    // ── Action Block Parsing ────────────────────────────────────────

    // Strips any remaining [ACTION] or [/ACTION] tags after extraction
    private static readonly Regex StrayActionTags = new(
        @"\[/?ACTION\]",
        RegexOptions.Compiled);

    /// <summary>
    /// Extracts balanced JSON objects following [ACTION] tags. Uses brace-depth counting
    /// instead of regex to handle nested arrays/objects in slot values (e.g. checklist arrays).
    /// </summary>
    private static (string CleanText, List<ParsedAction> Actions) ParseActionBlocks(string text)
    {
        var actions = new List<ParsedAction>();
        var spans = new List<(int Start, int End)>(); // regions to strip

        var searchFrom = 0;
        while (searchFrom < text.Length)
        {
            var tagIdx = text.IndexOf("[ACTION]", searchFrom, StringComparison.OrdinalIgnoreCase);
            if (tagIdx < 0) break;

            var afterTag = tagIdx + "[ACTION]".Length;

            // Find the opening brace
            var braceStart = -1;
            for (var i = afterTag; i < text.Length; i++)
            {
                if (text[i] == '{') { braceStart = i; break; }
                if (!char.IsWhiteSpace(text[i])) break; // non-whitespace before { means malformed
            }

            if (braceStart < 0) { searchFrom = afterTag; continue; }

            // Walk forward counting brace depth to find the matching close
            var depth = 0;
            var braceEnd = -1;
            var inString = false;
            var escaped = false;
            for (var i = braceStart; i < text.Length; i++)
            {
                var c = text[i];
                if (escaped) { escaped = false; continue; }
                if (c == '\\' && inString) { escaped = true; continue; }
                if (c == '"') { inString = !inString; continue; }
                if (inString) continue;
                if (c == '{') depth++;
                else if (c == '}') { depth--; if (depth == 0) { braceEnd = i; break; } }
            }

            if (braceEnd < 0) { searchFrom = afterTag; continue; }

            var json = text[(braceStart)..(braceEnd + 1)];

            // Find the end of the region to strip (include optional [/ACTION] tag)
            var regionEnd = braceEnd + 1;
            var remaining = text.AsSpan(regionEnd);
            var trimmed = remaining.TrimStart();
            if (trimmed.StartsWith("[/ACTION]", StringComparison.OrdinalIgnoreCase))
                regionEnd = text.Length - trimmed.Length + "[/ACTION]".Length;

            try
            {
                using var doc = JsonDocument.Parse(json);
                var root = doc.RootElement;
                var intentId = root.GetProperty("intent_id").GetString();
                var slots = new Dictionary<string, string>();

                if (root.TryGetProperty("slots", out var slotsEl))
                {
                    foreach (var prop in slotsEl.EnumerateObject())
                        slots[prop.Name] = FlattenSlotValue(prop.Value);
                }

                if (!string.IsNullOrEmpty(intentId))
                {
                    actions.Add(new ParsedAction(intentId, slots));
                    spans.Add((tagIdx, regionEnd));
                }
            }
            catch (Exception ex)
            {
                _log.Debug(ex, "Failed to parse action block JSON: {Json}", json);
            }

            searchFrom = braceEnd + 1;
        }

        // Strip matched regions in reverse order to preserve indices
        var clean = text;
        for (var i = spans.Count - 1; i >= 0; i--)
            clean = clean.Remove(spans[i].Start, spans[i].End - spans[i].Start);
        clean = StrayActionTags.Replace(clean, "");
        clean = clean.Trim();
        return (clean, actions);
    }

    /// <summary>
    /// Converts a JSON slot value to a flat string. Arrays are joined with newlines
    /// (supports checklist items, tags, etc. sent as JSON arrays by the AI).
    /// </summary>
    private static string FlattenSlotValue(JsonElement value)
    {
        return value.ValueKind switch
        {
            JsonValueKind.String => value.GetString() ?? "",
            JsonValueKind.Array => string.Join("\n", value.EnumerateArray()
                .Select(e => e.ValueKind == JsonValueKind.String ? e.GetString() ?? "" : e.GetRawText())),
            JsonValueKind.Number => value.GetRawText(),
            JsonValueKind.True => "true",
            JsonValueKind.False => "false",
            _ => value.GetRawText(),
        };
    }

    private sealed record ParsedAction(string IntentId, Dictionary<string, string> Slots);
}
