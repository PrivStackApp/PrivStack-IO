// ============================================================================
// File: EmbeddingSpaceViewModel.cs
// Description: ViewModel for the 3D embedding space visualization tab.
//              Manages loading state, similarity parameters, selection,
//              and entity type visibility.
// ============================================================================

using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Plugins.Graph.Services;
using PrivStack.Sdk;
using PrivStack.UI.Adaptive.Models;

namespace PrivStack.Desktop.Plugins.Graph.ViewModels;

public partial class EmbeddingSpaceViewModel : ViewModelBase
{
    private readonly EmbeddingDataService _dataService;
    private readonly IPluginSettings? _settings;
    private bool _isInitializing = true;

    [ObservableProperty] private EmbeddingSpaceData? _embeddingData;
    [ObservableProperty] private bool _isLoading;

    // Parameters
    [ObservableProperty] private double _similarityThreshold = 0.85;
    [ObservableProperty] private int _maxPoints = 1000;
    [ObservableProperty] private int _maxNeighbors = 5;
    [ObservableProperty] private bool _autoRotate = true;

    // Stats
    [ObservableProperty] private int _pointCount;
    [ObservableProperty] private int _edgeCount;

    // Selection
    [ObservableProperty] private int _selectedIndex = -1;
    [ObservableProperty] private string? _selectedTitle;
    [ObservableProperty] private string? _selectedEntityType;
    [ObservableProperty] private string? _selectedChunkText;

    // Entity type visibility
    [ObservableProperty] private bool _showNotes = true;
    [ObservableProperty] private bool _showTasks = true;
    [ObservableProperty] private bool _showContacts = true;
    [ObservableProperty] private bool _showEvents = true;
    [ObservableProperty] private bool _showJournal = true;
    [ObservableProperty] private bool _showSnippets = true;
    [ObservableProperty] private bool _showRss = true;
    [ObservableProperty] private bool _showFiles = true;

    // Events for the view to respond to
    public event EventHandler? RequestRefresh;
    public event EventHandler<bool>? AutoRotateChanged;

    public EmbeddingSpaceViewModel(EmbeddingDataService dataService, IPluginSettings? settings = null)
    {
        _dataService = dataService;
        _settings = settings;

        if (_settings != null)
        {
            _similarityThreshold = _settings.Get("emb_sim_threshold", 0.85);
            _maxPoints = _settings.Get("emb_max_points", 1000);
            _maxNeighbors = _settings.Get("emb_max_neighbors", 5);
            _autoRotate = _settings.Get("emb_auto_rotate", true);
        }

        _isInitializing = false;
    }

    [RelayCommand]
    public async Task LoadAsync()
    {
        IsLoading = true;
        try
        {
            var entityTypes = BuildEntityTypeFilter();
            EmbeddingData = await _dataService.LoadAsync(
                MaxPoints, SimilarityThreshold, MaxNeighbors, entityTypes);
            PointCount = EmbeddingData?.Points.Count ?? 0;
            EdgeCount = EmbeddingData?.Edges.Count ?? 0;
        }
        finally
        {
            IsLoading = false;
        }
    }

    [RelayCommand]
    private async Task RefreshAsync()
    {
        RequestRefresh?.Invoke(this, EventArgs.Empty);
        await LoadAsync();
    }

    public void OnPointClicked(int index)
    {
        if (EmbeddingData == null || index < 0 || index >= EmbeddingData.Points.Count) return;
        var point = EmbeddingData.Points[index];
        SelectedIndex = index;
        SelectedTitle = point.Title;
        SelectedEntityType = point.EntityType;
        SelectedChunkText = point.ChunkText;
    }

    public void OnPointDeselected()
    {
        SelectedIndex = -1;
        SelectedTitle = null;
        SelectedEntityType = null;
        SelectedChunkText = null;
    }

    private string[]? BuildEntityTypeFilter()
    {
        var types = new List<string>();
        if (ShowNotes) types.AddRange(["page", "sticky_note"]);
        if (ShowTasks) types.AddRange(["task", "project"]);
        if (ShowContacts) types.AddRange(["contact", "company", "contact_group"]);
        if (ShowEvents) types.Add("event");
        if (ShowJournal) types.Add("journal_entry");
        if (ShowSnippets) types.Add("snippet");
        if (ShowRss) types.Add("rss_article");
        if (ShowFiles) types.Add("vault_file");

        // If all types are selected, return null (no filter)
        if (ShowNotes && ShowTasks && ShowContacts && ShowEvents &&
            ShowJournal && ShowSnippets && ShowRss && ShowFiles)
            return null;

        return types.Count > 0 ? types.ToArray() : ["__none__"];
    }

    private void Save<T>(string key, T value) { if (!_isInitializing) _settings?.Set(key, value); }

    partial void OnSimilarityThresholdChanged(double value)
    {
        Save("emb_sim_threshold", value);
        if (!_isInitializing) _ = LoadAsync();
    }

    partial void OnMaxPointsChanged(int value)
    {
        Save("emb_max_points", value);
        if (!_isInitializing) _ = LoadAsync();
    }

    partial void OnMaxNeighborsChanged(int value)
    {
        Save("emb_max_neighbors", value);
        if (!_isInitializing) _ = LoadAsync();
    }

    partial void OnAutoRotateChanged(bool value)
    {
        Save("emb_auto_rotate", value);
        AutoRotateChanged?.Invoke(this, value);
    }

    partial void OnShowNotesChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowTasksChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowContactsChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowEventsChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowJournalChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowSnippetsChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowRssChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
    partial void OnShowFilesChanged(bool value) { if (!_isInitializing) _ = LoadAsync(); }
}
