using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Services.AI;
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

    [RelayCommand]
    private void StartNewChat()
    {
        // Current session is already persisted via AddMessage calls
        ActiveSessionId = null;
        EstimatedTokensUsed = 0;
        IsNearTokenLimit = false;
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
            var userName = _appSettings.Settings.UserDisplayName
                ?? Environment.UserName ?? "there";
            var tier = AiPersona.Classify(text);
            var isCloud = !IsActiveProviderLocal();

            // Semantic search across all indexed content for relevant context
            var ragContext = await BuildRagContextAsync(text);

            AiRequest request;
            if (isCloud)
            {
                var memoryContext = _memoryService.FormatForPrompt();
                var systemPrompt = AiPersona.GetCloudSystemPrompt(tier, userName, memoryContext);

                // Inject active plugin context
                if (!string.IsNullOrEmpty(_activePluginContext))
                    systemPrompt += $"\n\n{_activePluginContext}";

                // Inject RAG search results as knowledge context
                if (!string.IsNullOrEmpty(ragContext))
                    systemPrompt += $"\n\n{ragContext}";

                // Inject full entity context for cloud models (they can handle it)
                if (!string.IsNullOrEmpty(_activeItemContextFull))
                    systemPrompt += $"\n\n{_activeItemContextFull}";

                // Inject intent catalog so AI can invoke actions from chat
                var allIntents = _intentEngine.GetAllAvailableIntents();
                var intentCatalog = AiPersona.BuildIntentCatalog(allIntents);
                if (!string.IsNullOrEmpty(intentCatalog))
                {
                    systemPrompt += $"\n\n{intentCatalog}";
                    _log.Debug("Injected intent catalog with {IntentCount} actions into chat system prompt", allIntents.Count);
                }
                else
                {
                    _log.Debug("No intents available for chat intent catalog (providers: {Count})", allIntents.Count);
                }

                request = new AiRequest
                {
                    SystemPrompt = systemPrompt,
                    UserPrompt = text,
                    MaxTokens = AiPersona.CloudMaxTokensFor(tier),
                    Temperature = 0.4,
                    FeatureId = "tray.chat",
                    ConversationHistory = BuildConversationHistory()
                };
            }
            else
            {
                var systemPrompt = AiPersona.GetSystemPrompt(tier, userName);

                // Inject active plugin context
                if (!string.IsNullOrEmpty(_activePluginContext))
                    systemPrompt += $"\n\n{_activePluginContext}";

                // Inject RAG search results (trimmed for local models)
                if (!string.IsNullOrEmpty(ragContext))
                    systemPrompt += $"\n\n{ragContext}";

                // Inject short context for local models (limited context window)
                if (!string.IsNullOrEmpty(_activeItemContextShort))
                    systemPrompt += $"\n\n{_activeItemContextShort}";

                request = new AiRequest
                {
                    SystemPrompt = systemPrompt,
                    UserPrompt = text,
                    MaxTokens = AiPersona.MaxTokensFor(tier),
                    Temperature = 0.4,
                    FeatureId = "tray.chat",
                    ConversationHistory = BuildConversationHistory()
                };
            }

            AiResponse response;

            if (!isCloud)
            {
                // Stream tokens progressively for local models
                assistantMsg.State = ChatMessageState.Streaming;
                response = await _aiService.StreamCompleteAsync(request, partialContent =>
                {
                    _dispatcher.Post(() =>
                    {
                        assistantMsg.Content = AiPersona.Sanitize(partialContent, tier);
                    });
                });
            }
            else
            {
                response = await _aiService.CompleteAsync(request);
            }

            if (response.Success && !string.IsNullOrEmpty(response.Content))
            {
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

    private IReadOnlyList<AiChatMessage>? BuildConversationHistory()
    {
        var chatMessages = ChatMessages
            .Where(m => m.SuggestionId == null
                && !(m.Role == ChatMessageRole.Assistant && m.State == ChatMessageState.Loading))
            .TakeLast(MaxHistoryMessages)
            .ToList();

        if (chatMessages.Count == 0) return null;

        // Exclude the last user message (it's the one being sent now as UserPrompt)
        if (chatMessages.Count > 0 && chatMessages[^1].Role == ChatMessageRole.User)
            chatMessages.RemoveAt(chatMessages.Count - 1);

        if (chatMessages.Count == 0) return null;

        return chatMessages.Select(m => new AiChatMessage
        {
            Role = m.Role == ChatMessageRole.User ? "user" : "assistant",
            Content = m.Role == ChatMessageRole.User ? m.UserLabel ?? "" : m.Content ?? ""
        }).ToList();
    }

    /// <summary>
    /// Runs semantic search against the RAG vector index and formats matching results
    /// as context for the system prompt. Uses chunk text from the index for real content.
    /// </summary>
    private async Task<string?> BuildRagContextAsync(string query)
    {
        if (!_ragSearchService.IsReady)
            return null;

        try
        {
            var isCloud = !IsActiveProviderLocal();
            var limit = isCloud ? 10 : 8;
            var maxChunkChars = isCloud ? 500 : 800;

            var results = await _ragSearchService.SearchAsync(query, limit);

            if (results.Count == 0)
                return null;

            var relevant = results.Where(r => r.Score >= 0.3).ToList();
            if (relevant.Count == 0)
                return null;

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

            _log.Debug("RAG context: {Count} results injected into system prompt", relevant.Count);
            return sb.ToString().TrimEnd();
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "RAG search failed during chat, continuing without context");
            return null;
        }
    }

    // ── Action Block Parsing ────────────────────────────────────────

    private static readonly Regex ActionBlockPattern = new(
        @"\[ACTION\]\s*(\{.*?\})\s*\[/ACTION\]",
        RegexOptions.Singleline | RegexOptions.Compiled);

    private static (string CleanText, List<ParsedAction> Actions) ParseActionBlocks(string text)
    {
        var actions = new List<ParsedAction>();
        var matches = ActionBlockPattern.Matches(text);

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

        var clean = ActionBlockPattern.Replace(text, "").Trim();
        return (clean, actions);
    }

    private sealed record ParsedAction(string IntentId, Dictionary<string, string> Slots);
}
