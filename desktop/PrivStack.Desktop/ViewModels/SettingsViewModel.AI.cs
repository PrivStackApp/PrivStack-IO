using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.AI;
using PrivStack.Services.AI;
using PrivStack.Sdk;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// Broadcast when AI intent engine settings change (AiEnabled or AiIntentEnabled toggled).
/// </summary>
public sealed record IntentSettingsChangedMessage;

/// <summary>
/// Represents an AI provider option for the settings dropdown.
/// </summary>
public record AiProviderOption(string Id, string DisplayName, PrivacyTier? PrivacyTier = null);

/// <summary>
/// Represents a local AI model option for the settings dropdown.
/// </summary>
public record AiLocalModelOption(string Id, string DisplayName, string SizeText, bool IsDownloaded);

/// <summary>
/// Represents a saved API key entry for display in the settings panel.
/// </summary>
public record SavedApiKeyEntry(string ProviderId, string ProviderDisplayName, string KeyHint);

/// <summary>
/// Represents an AI memory entry for display and inline editing in the settings audit list.
/// </summary>
public sealed partial class AiMemoryDisplayItem : ObservableObject
{
    public string Id { get; }
    public string CategoryLabel { get; }
    public string DateLabel { get; }

    [ObservableProperty]
    private string _content;

    [ObservableProperty]
    private bool _isEditing;

    public AiMemoryDisplayItem(string id, string content, string categoryLabel, string dateLabel)
    {
        Id = id;
        _content = content;
        CategoryLabel = categoryLabel;
        DateLabel = dateLabel;
    }
}

/// <summary>
/// AI settings section of the Settings panel.
/// </summary>
public partial class SettingsViewModel
{
    // ── AI Properties ──────────────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowAiApiKeyInput))]
    [NotifyPropertyChangedFor(nameof(ShowAiCloudModelSelect))]
    [NotifyPropertyChangedFor(nameof(ShowAiLocalModelSection))]
    private bool _aiEnabled;

    [ObservableProperty]
    private string? _aiApiKey;

    [ObservableProperty]
    private string? _aiApiKeyStatus;

    [ObservableProperty]
    private string _aiApiKeySaveLabel = "Save";

    [ObservableProperty]
    private double _aiTemperature = 0.7;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowAiApiKeyInput))]
    [NotifyPropertyChangedFor(nameof(ShowAiCloudModelSelect))]
    [NotifyPropertyChangedFor(nameof(ShowAiLocalModelSection))]
    [NotifyPropertyChangedFor(nameof(CanDownloadAiLocalModel))]
    private AiProviderOption? _selectedAiProvider;

    [ObservableProperty]
    private AiModelInfo? _selectedAiCloudModel;

    [ObservableProperty]
    private string? _aiModelSaveStatus;

    private bool _isLoadingAiSettings;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanDownloadAiLocalModel))]
    [NotifyPropertyChangedFor(nameof(AiLocalModelDownloadLabel))]
    private AiLocalModelOption? _selectedAiLocalModel;

    [ObservableProperty]
    private bool _isAiLocalModelDownloading;

    [ObservableProperty]
    private double _aiLocalModelDownloadProgress;

    [ObservableProperty]
    private string? _aiLocalModelDownloadStatus;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanDownloadEmbeddingModel))]
    [NotifyPropertyChangedFor(nameof(EmbeddingModelDownloadLabel))]
    [NotifyPropertyChangedFor(nameof(IsEmbeddingModelDownloaded))]
    private bool _isEmbeddingModelDownloading;

    [ObservableProperty]
    private double _embeddingModelDownloadProgress;

    [ObservableProperty]
    private string? _embeddingModelDownloadStatus;

    [ObservableProperty]
    private bool _aiIntentEnabled;

    [ObservableProperty]
    private bool _aiIntentAutoAnalyze = true;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowCloudRecommendedBanner))]
    [NotifyPropertyChangedFor(nameof(FitnessTierColor))]
    [NotifyPropertyChangedFor(nameof(FitnessTierLabel))]
    private HardwareReport? _hardwareReport;

    [ObservableProperty]
    private AiRecommendation? _aiRecommendation;

    [ObservableProperty]
    private int _personalMemoryCount;

    [ObservableProperty]
    private int _dataMemoryCount;

    [ObservableProperty]
    private bool _isMemoryListVisible;

    public ObservableCollection<AiMemoryDisplayItem> AiMemories { get; } = [];

    public ObservableCollection<AiProviderOption> AiProviderOptions { get; } = [];
    public ObservableCollection<AiModelInfo> AiCloudModels { get; } = [];
    public ObservableCollection<AiLocalModelOption> AiLocalModels { get; } = [];
    public ObservableCollection<SavedApiKeyEntry> SavedApiKeys { get; } = [];

    // ── Computed Properties ────────────────────────────────────────────

    public bool ShowAiApiKeyInput =>
        AiEnabled && SelectedAiProvider is { Id: "openai" or "anthropic" or "gemini" or "mistral" or "groq" };

    public bool ShowAiCloudModelSelect =>
        AiEnabled && SelectedAiProvider is { Id: "openai" or "anthropic" or "gemini" or "mistral" or "groq" };

    public bool ShowCloudRecommendedBanner =>
        HardwareReport is { FitnessTier: FitnessTier.Yellow or FitnessTier.Red };

    public string FitnessTierColor => HardwareReport?.FitnessTier switch
    {
        FitnessTier.Green => "#4CAF50",
        FitnessTier.Yellow => "#FFC107",
        FitnessTier.Red => "#F44336",
        _ => "#888888"
    };

    public string FitnessTierLabel => HardwareReport?.FitnessTier switch
    {
        FitnessTier.Green => "Good",
        FitnessTier.Yellow => "Fair",
        FitnessTier.Red => "Limited",
        _ => "Unknown"
    };

    public bool ShowAiLocalModelSection =>
        AiEnabled && SelectedAiProvider is { Id: "local" };

    public bool CanDownloadAiLocalModel =>
        SelectedAiLocalModel != null && !SelectedAiLocalModel.IsDownloaded && !IsAiLocalModelDownloading;

    public string AiLocalModelDownloadLabel =>
        SelectedAiLocalModel?.IsDownloaded == true ? "Downloaded" : "Download Model";

    public string AiLocalModelRecommendation
    {
        get
        {
            var (_, reason) = PlatformDetector.RecommendLocalModel();
            return reason;
        }
    }

    public string AiRecommendedModelId
    {
        get
        {
            var (modelId, _) = PlatformDetector.RecommendLocalModel();
            return modelId;
        }
    }

    public bool IsEmbeddingModelDownloaded
    {
        get
        {
            try { return App.Services.GetRequiredService<EmbeddingModelManager>().IsModelDownloaded; }
            catch { return false; }
        }
    }

    public bool CanDownloadEmbeddingModel => !IsEmbeddingModelDownloaded && !IsEmbeddingModelDownloading;

    public string EmbeddingModelDownloadLabel => IsEmbeddingModelDownloaded ? "Downloaded" : "Download Model";

    public string EmbeddingModelSizeDisplay
    {
        get
        {
            try { return App.Services.GetRequiredService<EmbeddingModelManager>().ModelSizeDisplay; }
            catch { return "~260 MB"; }
        }
    }

    // ── Initialization ─────────────────────────────────────────────────

    private void LoadAiSettings()
    {
        _isLoadingAiSettings = true;
        try
        {
            var settings = _settingsService.Settings;
            AiEnabled = settings.AiEnabled;
            AiTemperature = settings.AiTemperature;

            // Populate provider options
            AiProviderOptions.Clear();
            AiProviderOptions.Add(new AiProviderOption("none", "None (Disabled)"));
            AiProviderOptions.Add(new AiProviderOption("openai", "OpenAI", PrivacyTier.StandardApi));
            AiProviderOptions.Add(new AiProviderOption("anthropic", "Anthropic — Privacy-First", PrivacyTier.HighPrivacy));
            AiProviderOptions.Add(new AiProviderOption("gemini", "Google Gemini", PrivacyTier.StandardApi));
            AiProviderOptions.Add(new AiProviderOption("mistral", "Mistral AI — Privacy-First", PrivacyTier.HighPrivacy));
            AiProviderOptions.Add(new AiProviderOption("groq", "Groq", PrivacyTier.StandardApi));
            AiProviderOptions.Add(new AiProviderOption("local", "Local — Privacy-First", PrivacyTier.HighPrivacy));

            SelectedAiProvider = AiProviderOptions.FirstOrDefault(p => p.Id == settings.AiProvider)
                                 ?? AiProviderOptions[0];

            // Populate local models
            RefreshAiLocalModels();

            // Load cloud models for active provider
            RefreshAiCloudModels();

            // Load cloud model selection — must come AFTER RefreshAiCloudModels
            if (!string.IsNullOrEmpty(settings.AiModel))
            {
                SelectedAiCloudModel = AiCloudModels.FirstOrDefault(m => m.Id == settings.AiModel);
            }

            // Load local model selection
            if (!string.IsNullOrEmpty(settings.AiLocalModel))
            {
                SelectedAiLocalModel = AiLocalModels.FirstOrDefault(m => m.Id == settings.AiLocalModel);
            }

            AiIntentEnabled = settings.AiIntentEnabled;
            AiIntentAutoAnalyze = settings.AiIntentAutoAnalyze;

            RefreshHardwareAssessment();
            RefreshMemoryCounts();
            LoadSavedApiKeys();
        }
        finally
        {
            _isLoadingAiSettings = false;
        }
    }

    private void RefreshHardwareAssessment()
    {
        try
        {
            AiRecommendation = PlatformDetector.GetFullRecommendation();
            HardwareReport = AiRecommendation.Hardware;
        }
        catch { /* detection may fail on unusual platforms */ }
    }

    private void RefreshAiCloudModels()
    {
        AiCloudModels.Clear();
        if (SelectedAiProvider == null) return;

        try
        {
            var aiService = App.Services.GetRequiredService<AiService>();
            var provider = aiService.GetProvider(SelectedAiProvider.Id);
            if (provider == null) return;

            foreach (var model in provider.AvailableModels)
                AiCloudModels.Add(model);

            SelectedAiCloudModel ??= AiCloudModels.FirstOrDefault();
        }
        catch { /* AI service may not be ready */ }
    }

    private void RefreshAiLocalModels()
    {
        AiLocalModels.Clear();
        try
        {
            var modelManager = App.Services.GetRequiredService<AiModelManager>();
            var (recommendedId, _) = PlatformDetector.RecommendLocalModel();
            foreach (var modelName in modelManager.AvailableModels)
            {
                var displayName = modelName == recommendedId
                    ? $"{modelName} (Recommended)"
                    : modelName;
                AiLocalModels.Add(new AiLocalModelOption(
                    modelName,
                    displayName,
                    modelManager.GetModelSizeDisplay(modelName),
                    modelManager.IsModelDownloaded(modelName)));
            }
        }
        catch { /* model manager may not be ready */ }
    }

    private static readonly Dictionary<string, (string DisplayName, string BlobId)> ProviderKeyMap = new()
    {
        ["openai"] = ("OpenAI", "openai-api-key"),
        ["anthropic"] = ("Anthropic", "anthropic-api-key"),
        ["gemini"] = ("Google Gemini", "gemini-api-key"),
        ["mistral"] = ("Mistral AI", "mistral-api-key"),
        ["groq"] = ("Groq", "groq-api-key"),
    };

    private void LoadSavedApiKeys()
    {
        SavedApiKeys.Clear();
        var hints = _settingsService.Settings.AiSavedKeyHints;
        foreach (var (providerId, hint) in hints)
        {
            var displayName = ProviderKeyMap.TryGetValue(providerId, out var info)
                ? info.DisplayName : providerId;
            SavedApiKeys.Add(new SavedApiKeyEntry(providerId, displayName, hint));
        }
    }

    // ── Change Handlers ────────────────────────────────────────────────

    partial void OnAiEnabledChanged(bool value)
    {
        if (_isLoadingAiSettings) return;
        _settingsService.Settings.AiEnabled = value;
        _settingsService.SaveDebounced();
        WeakReferenceMessenger.Default.Send(new IntentSettingsChangedMessage());
    }

    partial void OnSelectedAiProviderChanged(AiProviderOption? value)
    {
        if (value == null) return;
        if (!_isLoadingAiSettings)
        {
            _settingsService.Settings.AiProvider = value.Id;
            _settingsService.SaveDebounced();
        }

        // Reset API key display
        AiApiKey = null;
        AiApiKeyStatus = null;
        AiApiKeySaveLabel = "Save";

        RefreshAiCloudModels();
    }

    partial void OnSelectedAiCloudModelChanged(AiModelInfo? value)
    {
        if (value == null || _isLoadingAiSettings) return;
        _settingsService.Settings.AiModel = value.Id;
        _settingsService.SaveDebounced();
        ShowModelSaveStatus();
    }

    partial void OnSelectedAiLocalModelChanged(AiLocalModelOption? value)
    {
        if (value == null || _isLoadingAiSettings) return;
        _settingsService.Settings.AiLocalModel = value.Id;
        _settingsService.SaveDebounced();
    }

    partial void OnAiTemperatureChanged(double value)
    {
        if (_isLoadingAiSettings) return;
        _settingsService.Settings.AiTemperature = value;
        _settingsService.SaveDebounced();
    }

    partial void OnAiIntentEnabledChanged(bool value)
    {
        if (_isLoadingAiSettings) return;
        _settingsService.Settings.AiIntentEnabled = value;
        _settingsService.SaveDebounced();
        WeakReferenceMessenger.Default.Send(new IntentSettingsChangedMessage());
    }

    partial void OnAiIntentAutoAnalyzeChanged(bool value)
    {
        if (_isLoadingAiSettings) return;
        _settingsService.Settings.AiIntentAutoAnalyze = value;
        _settingsService.SaveDebounced();
    }

    private async void ShowModelSaveStatus()
    {
        AiModelSaveStatus = "Saved";
        await Task.Delay(2000);
        AiModelSaveStatus = null;
    }

    // ── Commands ───────────────────────────────────────────────────────

    [RelayCommand]
    private async Task SaveAiApiKeyAsync()
    {
        if (string.IsNullOrWhiteSpace(AiApiKey) || SelectedAiProvider == null)
            return;

        if (!ProviderKeyMap.TryGetValue(SelectedAiProvider.Id, out var keyInfo)) return;
        var blobId = keyInfo.BlobId;

        try
        {
            var sdk = App.Services.GetRequiredService<IPrivStackSdk>();

            // Ensure vault exists and is unlocked.
            // RequestVaultUnlockAsync handles both initialization (if needed) and unlock.
            var isUnlocked = await sdk.VaultIsUnlocked("ai-vault");
            if (!isUnlocked)
            {
                var unlocked = await sdk.RequestVaultUnlockAsync("ai-vault");
                if (!unlocked) { AiApiKeyStatus = "Vault unlock required"; return; }
            }

            var keyBytes = System.Text.Encoding.UTF8.GetBytes(AiApiKey.Trim());
            await sdk.VaultBlobStore("ai-vault", blobId, keyBytes);

            // Store hint (last 4 chars) for display
            var trimmedKey = AiApiKey.Trim();
            var hint = trimmedKey.Length >= 4 ? trimmedKey[^4..] : trimmedKey;
            _settingsService.Settings.AiSavedKeyHints[SelectedAiProvider.Id] = hint;
            _settingsService.SaveDebounced();

            AiApiKeyStatus = "API key saved to vault";
            AiApiKeySaveLabel = "Saved";
            AiApiKey = null;

            LoadSavedApiKeys();

            // Clear cached key in provider
            var aiService = App.Services.GetRequiredService<AiService>();
            var provider = aiService.GetProvider(SelectedAiProvider.Id);
            ClearProviderCachedKey(provider);
        }
        catch (Exception ex)
        {
            AiApiKeyStatus = $"Failed to save: {ex.Message}";
        }
    }

    private static void ClearProviderCachedKey(IAiProvider? provider)
    {
        switch (provider)
        {
            case OpenAiProvider oai: oai.ClearCachedKey(); break;
            case AnthropicProvider ant: ant.ClearCachedKey(); break;
            case GeminiProvider gem: gem.ClearCachedKey(); break;
            case OpenAiCompatibleProviderBase compat: compat.ClearCachedKey(); break;
        }
    }

    [RelayCommand]
    private async Task DownloadAiLocalModelAsync()
    {
        if (SelectedAiLocalModel == null || SelectedAiLocalModel.IsDownloaded)
            return;

        try
        {
            IsAiLocalModelDownloading = true;
            AiLocalModelDownloadStatus = $"Downloading {SelectedAiLocalModel.DisplayName}...";

            var modelManager = App.Services.GetRequiredService<AiModelManager>();
            modelManager.PropertyChanged += OnAiModelManagerPropertyChanged;

            var modelId = SelectedAiLocalModel.Id;
            await modelManager.DownloadModelAsync(modelId);

            AiLocalModelDownloadStatus = "Download complete — loading model...";
            AiLocalModelDownloadProgress = 100;
            RefreshAiLocalModels();

            // Re-select the model
            SelectedAiLocalModel = AiLocalModels.FirstOrDefault(m => m.Id == modelId);

            // Auto-select this model in settings if no local model was selected
            if (string.IsNullOrEmpty(_settingsService.Settings.AiLocalModel))
            {
                _settingsService.Settings.AiLocalModel = modelId;
                _settingsService.SaveDebounced();
            }

            // Pre-load the model into memory so it's ready for immediate use
            try
            {
                var aiService = App.Services.GetRequiredService<AiService>();
                if (aiService.GetProvider("local") is LocalLlamaProvider localProvider)
                {
                    await localProvider.PreloadModelAsync(
                        _settingsService.Settings.AiLocalModel ?? modelId);
                    AiLocalModelDownloadStatus = "Ready — model loaded";
                }
                else
                {
                    AiLocalModelDownloadStatus = "Download complete";
                }
            }
            catch (Exception ex)
            {
                Serilog.Log.Warning(ex, "Failed to pre-load model after download");
                AiLocalModelDownloadStatus = "Downloaded — will load on first use";
            }
        }
        catch (OperationCanceledException)
        {
            AiLocalModelDownloadStatus = "Download cancelled";
        }
        catch (Exception ex)
        {
            AiLocalModelDownloadStatus = $"Download failed: {ex.Message}";
        }
        finally
        {
            IsAiLocalModelDownloading = false;
            var modelManager = App.Services.GetRequiredService<AiModelManager>();
            modelManager.PropertyChanged -= OnAiModelManagerPropertyChanged;
        }
    }

    [RelayCommand]
    private async Task DeleteAiApiKeyAsync(SavedApiKeyEntry? entry)
    {
        if (entry == null) return;

        if (!ProviderKeyMap.TryGetValue(entry.ProviderId, out var info)) return;

        try
        {
            var sdk = App.Services.GetRequiredService<IPrivStackSdk>();

            var isUnlocked = await sdk.VaultIsUnlocked("ai-vault");
            if (!isUnlocked)
            {
                var unlocked = await sdk.RequestVaultUnlockAsync("ai-vault");
                if (!unlocked) { AiApiKeyStatus = "Vault unlock required"; return; }
            }

            try
            {
                await sdk.VaultBlobDelete("ai-vault", info.BlobId);
            }
            catch
            {
                // Blob may already be gone (data wipe, profile change) — that's fine,
                // we still need to clean up the settings hint below.
            }

            _settingsService.Settings.AiSavedKeyHints.Remove(entry.ProviderId);
            _settingsService.SaveDebounced();

            LoadSavedApiKeys();

            // Clear cached key in the provider
            var aiService = App.Services.GetRequiredService<AiService>();
            ClearProviderCachedKey(aiService.GetProvider(entry.ProviderId));

            AiApiKeyStatus = $"{entry.ProviderDisplayName} API key deleted";
        }
        catch (Exception ex)
        {
            AiApiKeyStatus = $"Failed to delete: {ex.Message}";
        }
    }

    [RelayCommand]
    private async Task DownloadEmbeddingModelAsync()
    {
        if (IsEmbeddingModelDownloaded) return;

        try
        {
            IsEmbeddingModelDownloading = true;
            EmbeddingModelDownloadStatus = "Downloading embedding model...";

            var modelManager = App.Services.GetRequiredService<EmbeddingModelManager>();
            modelManager.PropertyChanged += OnEmbeddingModelManagerPropertyChanged;

            await modelManager.DownloadModelAsync();

            EmbeddingModelDownloadStatus = "Download complete — initializing...";

            // Initialize the embedding service now that the model is available
            var embeddingService = App.Services.GetRequiredService<EmbeddingService>();
            await embeddingService.InitializeAsync();

            // Kick off full RAG index in the background
            var ragIndexService = App.Services.GetRequiredService<RagIndexService>();
            _ = Task.Run(() => ragIndexService.StartFullIndexAsync());

            EmbeddingModelDownloadStatus = "Ready — indexing started";
            OnPropertyChanged(nameof(IsEmbeddingModelDownloaded));
            OnPropertyChanged(nameof(CanDownloadEmbeddingModel));
            OnPropertyChanged(nameof(EmbeddingModelDownloadLabel));
        }
        catch (OperationCanceledException)
        {
            EmbeddingModelDownloadStatus = "Download cancelled";
        }
        catch (Exception ex)
        {
            EmbeddingModelDownloadStatus = $"Download failed: {ex.Message}";
        }
        finally
        {
            IsEmbeddingModelDownloading = false;
            var modelManager = App.Services.GetRequiredService<EmbeddingModelManager>();
            modelManager.PropertyChanged -= OnEmbeddingModelManagerPropertyChanged;
        }
    }

    [RelayCommand]
    private void DeleteEmbeddingModel()
    {
        try
        {
            var modelManager = App.Services.GetRequiredService<EmbeddingModelManager>();
            modelManager.DeleteModel();
            EmbeddingModelDownloadStatus = null;
            OnPropertyChanged(nameof(IsEmbeddingModelDownloaded));
            OnPropertyChanged(nameof(CanDownloadEmbeddingModel));
            OnPropertyChanged(nameof(EmbeddingModelDownloadLabel));
        }
        catch (Exception ex)
        {
            EmbeddingModelDownloadStatus = $"Delete failed: {ex.Message}";
        }
    }

    private void OnAiModelManagerPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(AiModelManager.DownloadProgress))
        {
            var modelManager = App.Services.GetRequiredService<AiModelManager>();
            AiLocalModelDownloadProgress = modelManager.DownloadProgress;
            AiLocalModelDownloadStatus = $"Downloading... {modelManager.DownloadProgress:F0}%";
        }
    }

    private void OnEmbeddingModelManagerPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(EmbeddingModelManager.DownloadProgress))
        {
            var modelManager = App.Services.GetRequiredService<EmbeddingModelManager>();
            EmbeddingModelDownloadProgress = modelManager.DownloadProgress;
            EmbeddingModelDownloadStatus = $"Downloading... {modelManager.DownloadProgress:F0}%";
        }
    }

    // ── AI Memory Management ──────────────────────────────────────────

    private void RefreshMemoryCounts()
    {
        try
        {
            var memoryService = App.Services.GetRequiredService<AiMemoryService>();
            PersonalMemoryCount = memoryService.PersonalMemoryCount;
            DataMemoryCount = memoryService.DataMemoryCount;
        }
        catch { /* service may not be ready */ }
    }

    private void RefreshMemoryList()
    {
        AiMemories.Clear();
        try
        {
            var memoryService = App.Services.GetRequiredService<AiMemoryService>();
            foreach (var m in memoryService.Memories)
            {
                var categoryLabel = AiMemoryService.IsPersonalCategory(m.Category) ? "Personal" : "Data";
                var dateLabel = m.CreatedAt.ToString("MMM d, yyyy");
                AiMemories.Add(new AiMemoryDisplayItem(m.Id, m.Content, categoryLabel, dateLabel));
            }
        }
        catch { /* service may not be ready */ }
    }

    [RelayCommand]
    private void ToggleMemoryList()
    {
        IsMemoryListVisible = !IsMemoryListVisible;
        if (IsMemoryListVisible)
            RefreshMemoryList();
    }

    [RelayCommand]
    private void DeleteMemory(AiMemoryDisplayItem? item)
    {
        if (item == null) return;

        var memoryService = App.Services.GetRequiredService<AiMemoryService>();
        memoryService.Remove(item.Id);
        AiMemories.Remove(item);
        RefreshMemoryCounts();
    }

    [RelayCommand]
    private void EditMemory(AiMemoryDisplayItem? item)
    {
        if (item == null) return;
        item.IsEditing = true;
    }

    [RelayCommand]
    private void SaveMemory(AiMemoryDisplayItem? item)
    {
        if (item == null) return;
        item.IsEditing = false;

        var memoryService = App.Services.GetRequiredService<AiMemoryService>();
        memoryService.Update(item.Id, item.Content);
    }

    [RelayCommand]
    private async Task ClearPersonalMemoriesAsync()
    {
        var confirmed = await _dialogService.ShowConfirmationAsync(
            "Clear Personal Memories",
            "This will delete all personal memories (preferences, personal facts) the AI has learned about you. This cannot be undone.",
            "Clear");

        if (!confirmed) return;

        var memoryService = App.Services.GetRequiredService<AiMemoryService>();
        memoryService.ClearPersonalMemories();
        RefreshMemoryCounts();
        if (IsMemoryListVisible) RefreshMemoryList();
    }

    [RelayCommand]
    private async Task ClearDataMemoriesAsync()
    {
        var confirmed = await _dialogService.ShowConfirmationAsync(
            "Clear Data Memories",
            "This will delete all data-derived memories (content from notes, tasks, and other plugins). This cannot be undone.",
            "Clear");

        if (!confirmed) return;

        var memoryService = App.Services.GetRequiredService<AiMemoryService>();
        memoryService.ClearDataMemories();
        RefreshMemoryCounts();
        if (IsMemoryListVisible) RefreshMemoryList();
    }

    [RelayCommand]
    private async Task ClearAllMemoriesAsync()
    {
        var confirmed = await _dialogService.ShowConfirmationAsync(
            "Clear All Memories",
            "This will delete all AI memories — both personal and data-derived. This cannot be undone.",
            "Clear All");

        if (!confirmed) return;

        var memoryService = App.Services.GetRequiredService<AiMemoryService>();
        memoryService.ClearAll();
        RefreshMemoryCounts();
        if (IsMemoryListVisible) RefreshMemoryList();
    }
}
