using System.Text;
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

                // Inject RAG search results as knowledge context
                if (!string.IsNullOrEmpty(ragContext))
                    systemPrompt += $"\n\n{ragContext}";

                // Inject full entity context for cloud models (they can handle it)
                if (!string.IsNullOrEmpty(_activeItemContextFull))
                    systemPrompt += $"\n\n{_activeItemContextFull}";

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
                    FeatureId = "tray.chat"
                };
            }

            var response = await _aiService.CompleteAsync(request);

            if (response.Success && !string.IsNullOrEmpty(response.Content))
            {
                var content = isCloud
                    ? response.Content
                    : AiPersona.Sanitize(response.Content, tier);

                assistantMsg.Content = content;
                assistantMsg.State = ChatMessageState.Ready;

                // Persist assistant message
                _conversationStore.AddMessage(ActiveSessionId!, "assistant", content);

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
            var limit = isCloud ? 10 : 5;
            var maxChunkChars = isCloud ? 500 : 200;

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
}
