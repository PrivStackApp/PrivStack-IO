using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.Input;

namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Conversation history tab — browse, load, and delete past sessions.
/// </summary>
public partial class AiSuggestionTrayViewModel
{
    public ObservableCollection<ConversationSessionViewModel> ConversationHistory { get; } = [];

    [RelayCommand]
    private void LoadConversation(ConversationSessionViewModel session)
    {
        var stored = _conversationStore.GetSession(session.Id);
        if (stored == null) return;

        ChatMessages.Clear();

        foreach (var msg in stored.Messages)
        {
            if (msg.Role == "user")
            {
                ChatMessages.Add(new AiChatMessageViewModel(ChatMessageRole.User)
                {
                    UserLabel = msg.Content
                });
            }
            else
            {
                ChatMessages.Add(new AiChatMessageViewModel(ChatMessageRole.Assistant)
                {
                    Content = msg.Content,
                    State = ChatMessageState.Ready
                });
            }
        }

        ActiveSessionId = session.Id;
        EstimatedTokensUsed = stored.EstimatedTokens;
        RefreshContextWindowSize();
        UpdateTokenEstimate();
        UpdateCounts();
        RefreshConversationHistory();
        SelectedTabIndex = 0; // Switch to Chat tab
        RequestScrollToBottom();
    }

    [RelayCommand]
    private void DeleteConversation(ConversationSessionViewModel session)
    {
        _conversationStore.DeleteSession(session.Id);

        if (ActiveSessionId == session.Id)
        {
            ActiveSessionId = null;
            ChatMessages.Clear();
            EstimatedTokensUsed = 0;
            IsNearTokenLimit = false;
            UpdateCounts();
        }

        RefreshConversationHistory();
    }

    internal void RefreshConversationHistory()
    {
        ConversationHistory.Clear();
        foreach (var session in _conversationStore.GetAllSessions())
        {
            ConversationHistory.Add(new ConversationSessionViewModel
            {
                Id = session.Id,
                Title = session.Title,
                TimeAgo = ConversationSessionViewModel.FormatTimeAgo(session.UpdatedAt),
                MessageCount = session.Messages.Count,
                IsActive = session.Id == ActiveSessionId
            });
        }
    }

    partial void OnSelectedTabIndexChanged(int value)
    {
        if (value == 2) // History tab
            RefreshConversationHistory();
    }
}
