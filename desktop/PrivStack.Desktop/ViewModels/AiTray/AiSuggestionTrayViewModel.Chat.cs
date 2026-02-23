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

                // Execute parsed intent actions from the AI response
                foreach (var action in actions)
                {
                    try
                    {
                        var result = await _intentEngine.ExecuteDirectAsync(
                            action.IntentId, action.Slots);
                        var confirmMsg = new AiChatMessageViewModel(ChatMessageRole.Assistant)
                        {
                            Content = result.Success
                                ? $"\u2713 {result.Summary ?? "Done!"}"
                                : $"\u2717 {result.ErrorMessage ?? "Action failed."}",
                            State = result.Success
                                ? ChatMessageState.Applied
                                : ChatMessageState.Error,
                        };
                        ChatMessages.Add(confirmMsg);
                        _conversationStore.AddMessage(ActiveSessionId!, "assistant", confirmMsg.Content!);

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
                        ChatMessages.Add(new AiChatMessageViewModel(ChatMessageRole.Assistant)
                        {
                            Content = $"\u2717 Failed to execute action: {ex.Message}",
                            State = ChatMessageState.Error,
                        });
                    }
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
        CRITICAL: You can perform REAL actions using [ACTION] blocks. Without an [ACTION] block, NOTHING happens.
        NEVER claim you created/did something unless you include the [ACTION] block below.
        Place [ACTION] blocks at the END of your response, after your conversational message.
        You may include multiple [ACTION] blocks for multiple actions.
        IMPORTANT: You MUST use the EXACT intent_id values shown in the ACTION descriptions above (e.g. "tasks.update_task", NOT "tasks.update").
        Do NOT abbreviate, shorten, or invent intent IDs. Copy them exactly from the action descriptions.
        You CAN use actions from ANY plugin, not just the one the user is currently viewing.
        For example, if the user asks to create a note while viewing Finance, use the notes.create_note action.
        If no relevant action exists in the descriptions above, say you can't do that yet.
        """;

    /// <summary>
    /// Runs semantic search against the RAG vector index and formats matching results.
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
            var limit = isCloud ? 10 : 8;
            var maxChunkChars = isCloud ? 500 : 800;

            var results = await _ragSearchService.SearchAsync(query, limit);

            if (results.Count == 0)
                return (null, false);

            var relevant = results.Where(r => r.Score >= 0.3).ToList();
            if (relevant.Count == 0)
                return (null, false);

            var hasIntentActions = relevant.Any(r => r.EntityType == "intent_action");
            _log.Debug("RAG context: {Count} results ({IntentCount} intent actions) injected into system prompt",
                relevant.Count, relevant.Count(r => r.EntityType == "intent_action"));

            var sb = new StringBuilder();
            sb.AppendLine("Relevant information from the user's data:");
            sb.AppendLine();

            foreach (var result in relevant)
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

    // ── Action Block Parsing ────────────────────────────────────────

    // Matches [ACTION]{...}[/ACTION] (properly closed)
    private static readonly Regex ClosedActionPattern = new(
        @"\[ACTION\]\s*(\{.*?\})\s*\[/ACTION\]",
        RegexOptions.Singleline | RegexOptions.Compiled);

    // Matches [ACTION]{...} without closing tag (terminated by next [ACTION], end of string, or [/ACTION])
    private static readonly Regex UnclosedActionPattern = new(
        @"\[ACTION\]\s*(\{[^[]*?\})(?=\s*(?:\[ACTION\]|\[/ACTION\]|$))",
        RegexOptions.Singleline | RegexOptions.Compiled);

    // Strips any remaining [ACTION] or [/ACTION] tags after extraction
    private static readonly Regex StrayActionTags = new(
        @"\[/?ACTION\]",
        RegexOptions.Compiled);

    private static (string CleanText, List<ParsedAction> Actions) ParseActionBlocks(string text)
    {
        var actions = new List<ParsedAction>();

        // Try closed blocks first, then fall back to unclosed
        var matches = ClosedActionPattern.Matches(text);
        if (matches.Count == 0)
            matches = UnclosedActionPattern.Matches(text);

        foreach (Match match in matches)
        {
            try
            {
                using var doc = JsonDocument.Parse(match.Groups[1].Value);
                var root = doc.RootElement;
                var intentId = root.GetProperty("intent_id").GetString();
                var slots = new Dictionary<string, string>();

                if (root.TryGetProperty("slots", out var slotsEl))
                {
                    foreach (var prop in slotsEl.EnumerateObject())
                        slots[prop.Name] = prop.Value.ValueKind == JsonValueKind.String
                            ? prop.Value.GetString() ?? ""
                            : prop.Value.GetRawText();
                }

                if (!string.IsNullOrEmpty(intentId))
                    actions.Add(new ParsedAction(intentId, slots));
            }
            catch (Exception ex)
            {
                _log.Debug(ex, "Failed to parse action block: {Block}", match.Value);
            }
        }

        // Strip all matched blocks and any stray tags from the clean text
        var clean = ClosedActionPattern.Replace(text, "");
        clean = UnclosedActionPattern.Replace(clean, "");
        clean = StrayActionTags.Replace(clean, "");
        clean = clean.Trim();
        return (clean, actions);
    }

    private sealed record ParsedAction(string IntentId, Dictionary<string, string> Slots);
}
