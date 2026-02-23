using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using Serilog;

namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Compact link picker for the AI chat input. Triggered by typing [[ in the chat box.
/// Searches all ILinkableItemProvider instances and returns results for selection.
/// </summary>
internal sealed partial class ChatLinkPickerViewModel : ObservableObject
{
    private static readonly ILogger _log = Log.ForContext<ChatLinkPickerViewModel>();
    private const int MaxResults = 8;
    private const int DebounceMs = 180;

    private readonly IPluginRegistry _pluginRegistry;
    private CancellationTokenSource? _searchCts;

    [ObservableProperty]
    private string _searchQuery = "";

    [ObservableProperty]
    private bool _isOpen;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private int _selectedIndex;

    public ObservableCollection<LinkableItem> Results { get; } = [];

    /// <summary>Fired when the user selects an item. Payload is the wiki-link string.</summary>
    public event Action<string>? ItemSelected;

    /// <summary>Fired when the picker should close without selection.</summary>
    public event Action? CloseRequested;

    public ChatLinkPickerViewModel(IPluginRegistry pluginRegistry)
    {
        _pluginRegistry = pluginRegistry;
    }

    public void Open()
    {
        SearchQuery = "";
        Results.Clear();
        SelectedIndex = 0;
        IsOpen = true;
        // Load recent/all items with empty query
        _ = SearchAsync("");
    }

    public void Close()
    {
        IsOpen = false;
        _searchCts?.Cancel();
        Results.Clear();
        CloseRequested?.Invoke();
    }

    public void SelectCurrent()
    {
        if (SelectedIndex >= 0 && SelectedIndex < Results.Count)
        {
            var item = Results[SelectedIndex];
            var wikiLink = $"[[{item.LinkType}:{item.Id}|{item.Title}]]";
            IsOpen = false;
            Results.Clear();
            ItemSelected?.Invoke(wikiLink);
        }
    }

    public void MoveUp()
    {
        if (Results.Count > 0)
            SelectedIndex = (SelectedIndex - 1 + Results.Count) % Results.Count;
    }

    public void MoveDown()
    {
        if (Results.Count > 0)
            SelectedIndex = (SelectedIndex + 1) % Results.Count;
    }

    partial void OnSearchQueryChanged(string value)
    {
        _ = DebouncedSearchAsync(value);
    }

    private async Task DebouncedSearchAsync(string query)
    {
        _searchCts?.Cancel();
        _searchCts = new CancellationTokenSource();
        var ct = _searchCts.Token;

        try
        {
            await Task.Delay(DebounceMs, ct);
            await SearchAsync(query);
        }
        catch (OperationCanceledException) { }
    }

    private async Task SearchAsync(string query)
    {
        try
        {
            IsLoading = true;
            var providers = _pluginRegistry.GetCapabilityProviders<ILinkableItemProvider>();
            var allResults = new List<LinkableItem>();

            foreach (var provider in providers)
            {
                try
                {
                    var items = await provider.SearchItemsAsync(
                        query, MaxResults, CancellationToken.None);
                    allResults.AddRange(items);
                }
                catch (Exception ex)
                {
                    _log.Debug(ex, "Search failed for provider {LinkType}", provider.LinkType);
                }
            }

            // Sort by title relevance then recency
            var sorted = allResults
                .OrderByDescending(i => !string.IsNullOrEmpty(query) &&
                    i.Title.Contains(query, StringComparison.OrdinalIgnoreCase) ? 1 : 0)
                .ThenByDescending(i => i.ModifiedAt ?? DateTime.MinValue)
                .Take(MaxResults)
                .ToList();

            Results.Clear();
            foreach (var item in sorted)
                Results.Add(item);

            SelectedIndex = Results.Count > 0 ? 0 : -1;
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Chat link picker search failed");
        }
        finally
        {
            IsLoading = false;
        }
    }
}
