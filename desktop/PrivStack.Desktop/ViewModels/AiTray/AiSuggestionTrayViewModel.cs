using System.Collections.ObjectModel;
using System.Text.Json;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Services.AI;
using PrivStack.Desktop.Services.AI;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
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
    IRecipient<ContentSuggestionRemovedMessage>,
    IRecipient<ContentSuggestionDismissedMessage>,
    IRecipient<ContentSuggestionActionRequestedMessage>
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
    private readonly IPrivStackSdk _sdk;
    private readonly RagSearchService _ragSearchService;
    private readonly IDatasetService _datasetService;

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
        IPluginRegistry pluginRegistry,
        IPrivStackSdk sdk,
        RagSearchService ragSearchService,
        IDatasetService datasetService)
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
        _sdk = sdk;
        _ragSearchService = ragSearchService;
        _datasetService = datasetService;

        // Subscribe to IntentEngine events
        _intentEngine.SuggestionAdded += OnIntentSuggestionAdded;
        _intentEngine.SuggestionRemoved += OnIntentSuggestionRemoved;
        _intentEngine.SuggestionsCleared += OnIntentSuggestionsCleared;

        // Subscribe to messenger messages
        WeakReferenceMessenger.Default.Register<IntentSettingsChangedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionPushedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionUpdatedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionRemovedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionDismissedMessage>(this);
        WeakReferenceMessenger.Default.Register<ContentSuggestionActionRequestedMessage>(this);

        // Subscribe to active item and plugin changes for context injection
        _infoPanelService.ActiveItemChanged += OnActiveItemChanged;
        _infoPanelService.ActivePluginChanged += OnActivePluginChanged;

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
    [NotifyPropertyChangedFor(nameof(IsChatTab))]
    [NotifyPropertyChangedFor(nameof(IsIntentsTab))]
    [NotifyPropertyChangedFor(nameof(IsHistoryTab))]
    private int _selectedTabIndex;

    public bool IsChatTab => SelectedTabIndex == 0;
    public bool IsIntentsTab => SelectedTabIndex == 1;
    public bool IsHistoryTab => SelectedTabIndex == 2;

    // ── Properties ───────────────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasCards))]
    private int _pendingCount;

    public bool HasCards => PendingCount > 0;

    [ObservableProperty]
    private bool _isOpen;

    [ObservableProperty]
    private bool _hasUnseenInsight;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasBalloonMessage))]
    private string? _balloonMessage;

    public bool HasBalloonMessage => !string.IsNullOrEmpty(BalloonMessage);

    private CancellationTokenSource? _balloonDismissCts;

    public bool IsEnabled => _appSettings.Settings.AiEnabled && _aiService.IsAvailable;

    /// <summary>Raised when the view should scroll to the bottom.</summary>
    public event EventHandler? ScrollToBottomRequested;

    /// <summary>Raised when the user clicks the reattach button in the detached window.</summary>
    public event EventHandler? ReattachRequested;

    [ObservableProperty]
    private bool _isDetached;

    [RelayCommand]
    private void RequestReattach() => ReattachRequested?.Invoke(this, EventArgs.Empty);

    // ── Active Item Context ──────────────────────────────────────────

    private string? _activeItemContextShort;
    private string? _activeItemContextFull;
    private string? _activePluginContext;
    private string? _activeItemRagKeywords;
    private bool _hasEmbeddedDatasets;

    private void OnActivePluginChanged()
    {
        var pluginId = _infoPanelService.ActivePluginId;
        if (string.IsNullOrEmpty(pluginId))
        {
            _activePluginContext = null;
            return;
        }

        var plugin = _pluginRegistry.ActivePlugins.FirstOrDefault(p => p.Metadata.Id == pluginId);
        if (plugin == null)
        {
            _activePluginContext = null;
            return;
        }

        // Minimal context line — full plugin details are in RAG index via IIndexableContentProvider
        _activePluginContext = $"The user is currently viewing the \"{plugin.Metadata.Name}\" plugin ({plugin.Metadata.Description}).";
    }

    private void OnActiveItemChanged()
    {
        var linkType = _infoPanelService.ActiveLinkType;
        var itemId = _infoPanelService.ActiveItemId;
        var title = _infoPanelService.ActiveItemTitle;

        if (string.IsNullOrEmpty(linkType) || string.IsNullOrEmpty(itemId) || string.IsNullOrEmpty(title))
        {
            _activeItemContextShort = null;
            _activeItemContextFull = null;
            _activeItemRagKeywords = null;
            _hasEmbeddedDatasets = false;
            return;
        }

        _hasEmbeddedDatasets = false;

        _activeItemContextShort = $"Currently viewing: {title} ({linkType})";
        _activeItemContextFull = _activeItemContextShort; // default until entity loads

        _ = FetchActiveItemEntityAsync(linkType, itemId, title);
    }

    private async Task FetchActiveItemEntityAsync(string linkType, string itemId, string title)
    {
        try
        {
            var displayName = EntityTypeMap.GetDisplayName(linkType) ?? linkType;
            string? json = null;

            // Try SDK entity read for mapped types
            var entityType = EntityTypeMap.GetEntityType(linkType);
            if (entityType != null)
            {
                json = await FetchEntityViaSDkAsync(entityType, itemId);

                // For notes, append embedded dataset/table content
                if (json != null && linkType == "page")
                    json = await AppendEmbeddedDatasetContentAsync(json, itemId);
            }

            // Fallback: query via IPluginDataSourceProvider for unmapped types (e.g. dataset_row)
            if (json == null)
                json = await FetchEntityViaDataSourceProviderAsync(linkType, itemId);

            if (_infoPanelService.ActiveItemId != itemId) return;

            if (json == null)
            {
                // Entity not fetchable via SDK or data source — use detail fields if available
                var details = _infoPanelService.ActiveItemDetails;
                if (details is { Count: > 0 })
                {
                    var detailLines = string.Join("\n", details.Select(d => $"  {d.Label}: {d.Value}"));
                    _activeItemContextFull =
                        $"The user is currently viewing: \"{title}\" ({displayName})\n{detailLines}";
                }
                return;
            }

            const int maxContextChars = 8000;
            if (json.Length > maxContextChars)
                json = json[..maxContextChars] + "\n... (truncated)";

            _activeItemContextFull =
                $"The user is currently viewing a {displayName} item: \"{title}\"\n" +
                $"Full entity data (JSON):\n```json\n{json}\n```";

            _activeItemRagKeywords = ExtractEntityKeywords(json, linkType);
        }
        catch (Exception ex)
        {
            _log.Debug(ex, "Failed to fetch active item entity for context: {LinkType}:{ItemId}", linkType, itemId);
        }
    }

    /// <summary>
    /// Extracts semantic keywords from entity JSON to augment RAG queries.
    /// Returns space-separated keywords, or null if none found.
    /// </summary>
    private static string? ExtractEntityKeywords(string json, string linkType)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            var keywords = new List<string>();

            switch (linkType)
            {
                case "contact":
                    AddJsonStringField(root, "job_title", keywords);
                    AddJsonStringField(root, "company_name", keywords);
                    AddJsonStringField(root, "department", keywords);
                    AddJsonStringField(root, "bio", keywords);
                    AddJsonArrayField(root, "tags", keywords);
                    break;

                case "task":
                    AddJsonArrayField(root, "tags", keywords);
                    AddJsonArrayField(root, "contexts", keywords);
                    AddJsonStringField(root, "project_name", keywords);
                    break;

                case "page":
                    AddJsonArrayField(root, "tags", keywords);
                    break;

                default:
                    AddJsonArrayField(root, "tags", keywords);
                    break;
            }

            return keywords.Count > 0 ? string.Join(" ", keywords) : null;
        }
        catch
        {
            return null;
        }
    }

    private static void AddJsonStringField(JsonElement root, string fieldName, List<string> keywords)
    {
        if (root.TryGetProperty(fieldName, out var prop) &&
            prop.ValueKind == JsonValueKind.String)
        {
            var value = prop.GetString();
            if (!string.IsNullOrWhiteSpace(value))
                keywords.Add(value);
        }
    }

    private static void AddJsonArrayField(JsonElement root, string fieldName, List<string> keywords)
    {
        if (root.TryGetProperty(fieldName, out var prop) &&
            prop.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in prop.EnumerateArray())
            {
                if (item.ValueKind == JsonValueKind.String)
                {
                    var value = item.GetString();
                    if (!string.IsNullOrWhiteSpace(value))
                        keywords.Add(value);
                }
            }
        }
    }

    private async Task<string?> FetchEntityViaSDkAsync(string entityType, string itemId)
    {
        var response = await _sdk.SendAsync<JsonElement>(new SdkMessage
        {
            PluginId = "privstack.graph",
            Action = SdkAction.Read,
            EntityType = entityType,
            EntityId = itemId,
        });

        if (!response.Success || response.Data.ValueKind == JsonValueKind.Undefined) return null;
        return JsonSerializer.Serialize(response.Data, new JsonSerializerOptions { WriteIndented = true });
    }

    private async Task<string?> FetchEntityViaDataSourceProviderAsync(string linkType, string itemId)
    {
        var providers = _pluginRegistry.GetCapabilityProviders<PrivStack.Sdk.Capabilities.IPluginDataSourceProvider>();
        var provider = providers.FirstOrDefault(p => p.NavigationLinkType == linkType);
        if (provider == null) return null;

        try
        {
            // Query the provider filtering by ID — fetch a small page and serialize
            var result = await provider.QueryItemAsync("all", page: 0, pageSize: 50, filterText: itemId);
            if (result.Rows.Count == 0) return null;

            // Build a readable representation: column headers + matching rows
            var sb = new System.Text.StringBuilder();
            sb.AppendLine($"Columns: {string.Join(", ", result.Columns)}");
            foreach (var row in result.Rows)
            {
                var fields = new List<string>();
                for (var i = 0; i < Math.Min(row.Count, result.Columns.Count); i++)
                    fields.Add($"{result.Columns[i]}: {row[i]}");
                sb.AppendLine(string.Join(", ", fields));
            }
            return sb.ToString();
        }
        catch (Exception ex)
        {
            _log.Debug(ex, "DataSourceProvider query failed for {LinkType}:{ItemId}", linkType, itemId);
            return null;
        }
    }

    /// <summary>
    /// Scans the note's content blocks for embedded table/chart blocks backed by
    /// datasets or cross-plugin queries, fetches a sample of their data, and
    /// appends it to the context so Duncan can answer questions about them.
    /// </summary>
    private async Task<string> AppendEmbeddedDatasetContentAsync(string noteJson, string itemId)
    {
        try
        {
            using var doc = JsonDocument.Parse(noteJson);
            var root = doc.RootElement;

            // Page JSON structure: { "content": { "type": "doc", "content": [...blocks...] } }
            // Drill through the PageDocument wrapper to reach the block array.
            if (!root.TryGetProperty("content", out var contentField))
                return noteJson;

            // Find the block array — could be at content.content (PageDocument) or content directly
            JsonElement? blocksArray = null;
            if (contentField.ValueKind == JsonValueKind.Object
                && contentField.TryGetProperty("content", out var inner)
                && inner.ValueKind == JsonValueKind.Array)
            {
                blocksArray = inner;
            }
            else if (contentField.ValueKind == JsonValueKind.Array)
            {
                blocksArray = contentField;
            }

            List<EmbeddedDataRef> refs;
            if (blocksArray.HasValue)
                refs = ExtractDataRefsFromBlocks(blocksArray.Value);
            else if (contentField.ValueKind == JsonValueKind.String)
                refs = ExtractDataRefsFromString(contentField.GetString() ?? "");
            else
                return noteJson;

            if (refs.Count == 0) return noteJson;

            var providers = _pluginRegistry.GetCapabilityProviders<PrivStack.Sdk.Capabilities.IPluginDataSourceProvider>();
            var sb = new System.Text.StringBuilder(noteJson);

            foreach (var dataRef in refs.Take(3))
            {
                try
                {
                    PrivStack.Sdk.Capabilities.DatasetQueryResult? result = null;

                    if (dataRef.BackingMode == "plugin_query"
                        && dataRef.ProviderPluginId != null
                        && dataRef.ProviderQueryKey != null)
                    {
                        // Cross-plugin query (Tasks, Contacts, etc.)
                        var provider = providers.FirstOrDefault(p =>
                            p.PluginId == dataRef.ProviderPluginId);
                        if (provider != null)
                            result = await provider.QueryItemAsync(dataRef.ProviderQueryKey, page: 0, pageSize: 20);
                    }
                    else if (dataRef.DatasetId != null)
                    {
                        // Dataset-backed (Data plugin)
                        var dataProvider = providers.FirstOrDefault(p => p.NavigationLinkType == "dataset_row");
                        if (dataProvider != null)
                            result = await dataProvider.QueryItemAsync(dataRef.DatasetId, page: 0, pageSize: 20);
                    }

                    if (result == null || result.Rows.Count == 0) continue;

                    var label = dataRef.Title ?? dataRef.DatasetId ?? dataRef.ProviderQueryKey ?? "unknown";
                    sb.AppendLine();
                    sb.AppendLine($"\n--- Embedded Table: {label} ({result.TotalCount} total rows) ---");

                    // If dataset-backed, fetch full schema metadata for SQL querying
                    if (dataRef.DatasetId != null)
                    {
                        var datasetInfo = await _datasetService.GetDatasetAsync(dataRef.DatasetId);
                        if (datasetInfo != null)
                        {
                            _hasEmbeddedDatasets = true;
                            sb.AppendLine($"Dataset ID: {datasetInfo.Id.Value} (use this in chart block dataset_id)");
                            sb.AppendLine($"Dataset Name: {datasetInfo.Name} (use source:\"{datasetInfo.Name}\" in SQL)");
                            sb.AppendLine($"Total Rows: {datasetInfo.RowCount}");
                            sb.AppendLine("Schema:");
                            foreach (var col in datasetInfo.Columns)
                                sb.AppendLine($"  - {col.Name} ({col.ColumnType})");
                        }
                    }

                    sb.AppendLine($"Columns: {string.Join(", ", result.Columns)}");
                    foreach (var row in result.Rows)
                    {
                        var fields = new List<string>();
                        for (var i = 0; i < Math.Min(row.Count, result.Columns.Count); i++)
                            fields.Add($"{result.Columns[i]}: {row[i]}");
                        sb.AppendLine(string.Join(" | ", fields));
                    }
                    if (result.TotalCount > result.Rows.Count)
                        sb.AppendLine($"... ({result.TotalCount - result.Rows.Count} more rows)");
                }
                catch (Exception ex)
                {
                    _log.Debug(ex, "Failed to fetch embedded data for note {NoteId}: {Ref}", itemId, dataRef);
                }
            }
            return sb.ToString();
        }
        catch
        {
            return noteJson;
        }
    }

    /// <summary>
    /// Extracts dataset/plugin-query references from structured block array content.
    /// </summary>
    private static List<EmbeddedDataRef> ExtractDataRefsFromBlocks(JsonElement blocksArray)
    {
        var refs = new List<EmbeddedDataRef>();
        foreach (var block in blocksArray.EnumerateArray())
        {
            if (!block.TryGetProperty("type", out var typeProp)) continue;
            var type = typeProp.GetString();
            if (type is not ("table" or "chart")) continue;

            var backingMode = block.TryGetProperty("backing_mode", out var bm) ? bm.GetString() : null;
            var datasetId = block.TryGetProperty("dataset_id", out var ds) ? ds.GetString() : null;
            var providerPluginId = block.TryGetProperty("provider_plugin_id", out var pp) ? pp.GetString() : null;
            var providerQueryKey = block.TryGetProperty("provider_query_key", out var pq) ? pq.GetString() : null;
            var title = block.TryGetProperty("title", out var t) ? t.GetString() : null;

            // Only include blocks that reference external data
            if (datasetId != null || (providerPluginId != null && providerQueryKey != null))
            {
                refs.Add(new EmbeddedDataRef(
                    backingMode, datasetId, providerPluginId, providerQueryKey, title));
            }
        }
        return refs;
    }

    /// <summary>
    /// Legacy fallback: extracts dataset_id values from a content string via text scanning.
    /// </summary>
    private static List<EmbeddedDataRef> ExtractDataRefsFromString(string contentStr)
    {
        var refs = new List<EmbeddedDataRef>();
        var idx = 0;
        while ((idx = contentStr.IndexOf("dataset_id", idx, StringComparison.Ordinal)) >= 0)
        {
            var start = contentStr.IndexOf('"', idx + 10);
            if (start < 0) { idx++; continue; }
            start++;
            var end = contentStr.IndexOf('"', start);
            if (end > start && end - start < 100)
                refs.Add(new EmbeddedDataRef("dataset", contentStr[start..end], null, null, null));
            idx = end > 0 ? end : idx + 1;
        }
        return refs;
    }

    private sealed record EmbeddedDataRef(
        string? BackingMode, string? DatasetId,
        string? ProviderPluginId, string? ProviderQueryKey, string? Title);

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
