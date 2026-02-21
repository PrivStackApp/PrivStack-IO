using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.AI;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Messaging;
using PrivStack.Sdk.Services;
using Serilog;

namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Unified ViewModel for the global AI chat tray.
/// Split into partial files: Chat, Intents, History.
/// </summary>
public partial class AiSuggestionTrayViewModel : ViewModelBase,
    IRecipient<IntentSettingsChangedMessage>,
    IRecipient<ContentSuggestionPushedMessage>,
    IRecipient<ContentSuggestionUpdatedMessage>,
    IRecipient<ContentSuggestionRemovedMessage>
{
    private static readonly ILogger _log = Serilog.Log.ForContext<AiSuggestionTrayViewModel>();

    private readonly IIntentEngine _intentEngine;
    private readonly IUiDispatcher _dispatcher;
    private readonly IAppSettingsService _appSettings;
    internal readonly AiService _aiService;
    private readonly AiMemoryService _memoryService;
    private readonly AiMemoryExtractor _memoryExtractor;
    private readonly AiConversationStore _conversationStore;
    private readonly InfoPanelService _infoPanelService;
    private readonly IPluginRegistry _pluginRegistry;

    /// <summary>
    /// Set by MainWindowViewModel to enable source entity navigation without coupling.
    /// </summary>
    public Func<string, string, Task>? NavigateToLinkedItemFunc { get; set; }

    internal AiSuggestionTrayViewModel(
        IIntentEngine intentEngine,
        IUiDispatcher dispatcher,
        IAppSettingsService appSettings,
        AiService aiService,
        AiMemoryService memoryService,
        AiMemoryExtractor memoryExtractor,
        AiConversationStore conversationStore,
        InfoPanelService infoPanelService,
        IPluginRegistry pluginRegistry)
    {
        _intentEngine = intentEngine;
        _dispatcher = dispatcher;
        _appSettings = appSettings;
        _aiService = aiService;
        _memoryService = memoryService;
        _memoryExtractor = memoryExtractor;
        _conversationStore = conversationStore;
        _infoPanelService = infoPanelService;
        _pluginRegistry = pluginRegistry;

        // Subscribe to IntentEngine events
        _intentEngine.SuggestionAdded += OnIntentSuggestionAdded;
        _intentEngine.SuggestionRemoved += OnIntentSuggestionRemoved;
        _intentEngine.SuggestionsCleared += OnIntentSuggestionsCleared;

        // Subscribe to messenger messages
        WeakReferenceMessenger.Default.Register<IntentSettingsChangedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionPushedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionUpdatedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionRemovedMessage>(this);

        // Subscribe to active item changes for context injection
        _infoPanelService.ActiveItemChanged += OnActiveItemChanged;

        // Load existing intent suggestions
        foreach (var suggestion in _intentEngine.PendingSuggestions)
            AddIntentAsAssistantMessage(suggestion);
        UpdateCounts();
    }

    // ── Collections ──────────────────────────────────────────────────

    public ObservableCollection<AiChatMessageViewModel> ChatMessages { get; } = [];
    public ObservableCollection<AiChatMessageViewModel> IntentMessages { get; } = [];

    // ── Tab Selection ────────────────────────────────────────────────

    [ObservableProperty]
    private int _selectedTabIndex;

    // ── Properties ───────────────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasCards))]
    private int _pendingCount;

    public bool HasCards => PendingCount > 0;

    [ObservableProperty]
    private bool _isOpen;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasBalloonMessage))]
    private string? _balloonMessage;

    public bool HasBalloonMessage => !string.IsNullOrEmpty(BalloonMessage);

    private CancellationTokenSource? _balloonDismissCts;

    public bool IsEnabled => _appSettings.Settings.AiEnabled && _aiService.IsAvailable;

    /// <summary>Raised when the view should scroll to the bottom.</summary>
    public event EventHandler? ScrollToBottomRequested;

    // ── Active Item Context ──────────────────────────────────────────

    private string? _activeItemContextBlock;

    private void OnActiveItemChanged()
    {
        _activeItemContextBlock = BuildActiveItemContext();
    }

    private string? BuildActiveItemContext()
    {
        var linkType = _infoPanelService.ActiveLinkType;
        var itemId = _infoPanelService.ActiveItemId;
        var title = _infoPanelService.ActiveItemTitle;

        if (string.IsNullOrEmpty(linkType) || string.IsNullOrEmpty(itemId) || string.IsNullOrEmpty(title))
            return null;

        var details = _infoPanelService.ActiveItemDetails;
        if (details is { Count: > 0 })
        {
            var fields = string.Join(", ", details.Select(d => $"{d.Label}: {d.Value}"));
            return $"The user is currently viewing: {title} ({linkType}). Details: {fields}";
        }

        return $"The user is currently viewing: {title} ({linkType})";
    }

    // ── Commands ─────────────────────────────────────────────────────

    [RelayCommand]
    private void Toggle()
    {
        IsOpen = !IsOpen;
        if (IsOpen) BalloonMessage = null;
    }

    [RelayCommand]
    private void DismissBalloon() => BalloonMessage = null;

    [RelayCommand]
    private void ClearAll()
    {
        _intentEngine.ClearAll();

        var contentMsgs = IntentMessages
            .Where(m => m is { Role: ChatMessageRole.Assistant, SuggestionId: not null, SourcePluginId: not null })
            .ToList();
        foreach (var msg in contentMsgs)
            msg.DismissCommand.Execute(null);

        ChatMessages.Clear();
        IntentMessages.Clear();
        _suggestionToAssistantId.Clear();
        _suggestionToUserMsgId.Clear();
        UpdateCounts();
    }

    // ── Messenger Handler ────────────────────────────────────────────

    public void Receive(IntentSettingsChangedMessage message)
    {
        _dispatcher.Post(() => OnPropertyChanged(nameof(IsEnabled)));
    }

    // ── Internals ────────────────────────────────────────────────────

    private void UpdateCounts()
    {
        PendingCount = ChatMessages.Count + IntentMessages.Count;
    }

    internal void RequestScrollToBottom()
    {
        ScrollToBottomRequested?.Invoke(this, EventArgs.Empty);
    }

    private void ShowBalloon(string message)
    {
        _balloonDismissCts?.Cancel();
        BalloonMessage = message;

        var cts = new CancellationTokenSource();
        _balloonDismissCts = cts;

        _ = Task.Delay(TimeSpan.FromSeconds(6), cts.Token).ContinueWith(_ =>
        {
            _dispatcher.Post(() => BalloonMessage = null);
        }, TaskContinuationOptions.OnlyOnRanToCompletion);
    }
}
