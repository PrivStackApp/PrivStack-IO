using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Messaging;
using PrivStack.Sdk.Services;
using Serilog;

namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Intent and content suggestion handlers — operates on <see cref="IntentMessages"/>.
/// </summary>
public partial class AiSuggestionTrayViewModel
{
    private const string IntentPrefix = "intent:";

    /// <summary>Maps SuggestionId → Assistant MessageId for update routing.</summary>
    private readonly Dictionary<string, string> _suggestionToAssistantId = new();

    /// <summary>Maps SuggestionId → User MessageId for removal.</summary>
    private readonly Dictionary<string, string> _suggestionToUserMsgId = new();

    // ── Intent Engine Event Handlers ─────────────────────────────────

    private void OnIntentSuggestionAdded(object? sender, IntentSuggestion suggestion)
    {
        _dispatcher.Post(() =>
        {
            AddIntentAsAssistantMessage(suggestion);
            UpdateCounts();
            HasUnseenInsight = true;
            ShowBalloon($"I noticed something: {suggestion.Summary}");
        });
    }

    private void OnIntentSuggestionRemoved(object? sender, string suggestionId)
    {
        _dispatcher.Post(() =>
        {
            RemoveMessageBySuggestionId(suggestionId);
            UpdateCounts();
        });
    }

    private void OnIntentSuggestionsCleared(object? sender, EventArgs e)
    {
        _dispatcher.Post(() =>
        {
            var intentMsgs = IntentMessages
                .Where(m => m.SuggestionId?.StartsWith(IntentPrefix) == true).ToList();
            foreach (var msg in intentMsgs)
                IntentMessages.Remove(msg);
            UpdateCounts();
        });
    }

    private void AddIntentAsAssistantMessage(IntentSuggestion suggestion)
    {
        var assistantMsg = new AiChatMessageViewModel(ChatMessageRole.Assistant)
        {
            SuggestionId = $"{IntentPrefix}{suggestion.SuggestionId}",
            Content = suggestion.Summary,
            State = ChatMessageState.Ready,
            SourcePluginId = suggestion.MatchedIntent.PluginId,
            SourceEntityId = suggestion.SourceSignal.EntityId,
            SourceEntityType = suggestion.SourceSignal.EntityType,
            SourceEntityTitle = suggestion.SourceSignal.EntityTitle,
            NavigateToSourceFunc = NavigateToLinkedItemFunc
        };

        // Add the primary action button (e.g. "Create Calendar Event", "Create Task")
        assistantMsg.Actions.Add(new SuggestionAction
        {
            ActionId = "execute_intent",
            DisplayName = suggestion.MatchedIntent.DisplayName,
            IsPrimary = true
        });

        IntentMessages.Add(assistantMsg);
        RequestScrollToBottom();
    }

    // ── Intent Action Execution ───────────────────────────────────────

    public void Receive(ContentSuggestionActionRequestedMessage message)
    {
        // Only handle intent suggestions here; plugin content suggestions are handled by the plugins themselves
        if (!message.SuggestionId.StartsWith(IntentPrefix)) return;

        var rawId = message.SuggestionId[IntentPrefix.Length..];

        if (message.ActionId == "execute_intent")
            _dispatcher.Post(() => _ = ExecuteIntentActionAsync(message.SuggestionId, rawId));
    }

    private async Task ExecuteIntentActionAsync(string prefixedId, string rawSuggestionId)
    {
        var msg = IntentMessages.FirstOrDefault(m => m.SuggestionId == prefixedId);
        if (msg != null)
        {
            msg.State = ChatMessageState.Loading;
            msg.Actions.Clear();
        }

        try
        {
            var result = await _intentEngine.ExecuteAsync(rawSuggestionId);

            if (result.Success)
            {
                if (msg != null)
                {
                    msg.State = ChatMessageState.Applied;
                    msg.Content = result.Summary ?? "Done!";
                }
            }
            else
            {
                if (msg != null)
                {
                    msg.State = ChatMessageState.Error;
                    msg.ErrorMessage = result.ErrorMessage ?? "Action failed.";
                }
            }
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to execute intent {SuggestionId}", rawSuggestionId);
            if (msg != null)
            {
                msg.State = ChatMessageState.Error;
                msg.ErrorMessage = ex.Message;
            }
        }

        UpdateCounts();
    }

    // ── Content Suggestion Messenger Handlers ────────────────────────

    public void Receive(ContentSuggestionPushedMessage message)
    {
        _dispatcher.Post(() =>
        {
            var card = message.Card;

            var userLabel = card.UserPromptLabel ?? $"Hey {AiPersona.Name}, {card.Title}";
            var userMsg = new AiChatMessageViewModel(ChatMessageRole.User)
            {
                SuggestionId = card.SuggestionId,
                UserLabel = userLabel,
                SourceEntityId = card.SourceEntityId,
                SourceEntityType = card.SourceEntityType,
                SourceEntityTitle = card.SourceEntityTitle,
                SourcePluginId = card.PluginId,
                NavigateToSourceFunc = NavigateToLinkedItemFunc
            };
            IntentMessages.Add(userMsg);
            _suggestionToUserMsgId[card.SuggestionId] = userMsg.MessageId;

            var assistantMsg = new AiChatMessageViewModel(ChatMessageRole.Assistant)
            {
                SuggestionId = card.SuggestionId,
                Content = card.Content,
                State = AiChatMessageViewModel.MapState(card.State),
                SourcePluginId = card.PluginId,
                SourceEntityId = card.SourceEntityId,
                SourceEntityType = card.SourceEntityType,
                SourceEntityTitle = card.SourceEntityTitle,
                NavigateToSourceFunc = NavigateToLinkedItemFunc
            };
            foreach (var action in card.Actions)
                assistantMsg.Actions.Add(action);

            IntentMessages.Add(assistantMsg);
            _suggestionToAssistantId[card.SuggestionId] = assistantMsg.MessageId;

            UpdateCounts();
            RequestScrollToBottom();

            if (card.State == ContentSuggestionState.Loading)
                ShowBalloon("Working on your request...");
        });
    }

    public void Receive(ContentSuggestionUpdatedMessage message)
    {
        _dispatcher.Post(() =>
        {
            if (!_suggestionToAssistantId.TryGetValue(message.SuggestionId, out var assistantMsgId))
                return;

            var assistantMsg = IntentMessages.FirstOrDefault(m => m.MessageId == assistantMsgId);
            if (assistantMsg == null) return;

            assistantMsg.ApplyUpdate(message);

            if (message.NewState == ContentSuggestionState.Ready)
                ShowBalloon("Your result is ready!");
        });
    }

    public void Receive(ContentSuggestionRemovedMessage message)
    {
        _dispatcher.Post(() =>
        {
            RemoveMessageBySuggestionId(message.SuggestionId);
            UpdateCounts();
        });
    }

    public void Receive(ContentSuggestionDismissedMessage message)
    {
        _dispatcher.Post(() =>
        {
            // Dismiss from IntentEngine if this is an intent suggestion
            if (message.SuggestionId.StartsWith(IntentPrefix))
                _intentEngine.Dismiss(message.SuggestionId[IntentPrefix.Length..]);

            RemoveMessageBySuggestionId(message.SuggestionId);
            UpdateCounts();
        });
    }

    private void RemoveMessageBySuggestionId(string suggestionId)
    {
        // Content suggestion cleanup (keyed by raw SuggestionId)
        if (_suggestionToUserMsgId.TryGetValue(suggestionId, out var userMsgId))
        {
            var userMsg = IntentMessages.FirstOrDefault(m => m.MessageId == userMsgId);
            if (userMsg != null) IntentMessages.Remove(userMsg);
            _suggestionToUserMsgId.Remove(suggestionId);
        }

        if (_suggestionToAssistantId.TryGetValue(suggestionId, out var assistantMsgId))
        {
            var assistantMsg = IntentMessages.FirstOrDefault(m => m.MessageId == assistantMsgId);
            if (assistantMsg != null) IntentMessages.Remove(assistantMsg);
            _suggestionToAssistantId.Remove(suggestionId);
        }

        // Intent suggestion cleanup — match by SuggestionId directly (handles both
        // prefixed "intent:{id}" from dismiss and raw "{id}" from IntentEngine events)
        var intentMsg = IntentMessages.FirstOrDefault(m => m.SuggestionId == suggestionId)
                     ?? IntentMessages.FirstOrDefault(m => m.SuggestionId == $"{IntentPrefix}{suggestionId}");
        if (intentMsg != null) IntentMessages.Remove(intentMsg);
    }
}
