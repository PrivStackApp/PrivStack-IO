using System.Text.Json;
using CommunityToolkit.Mvvm.Messaging;
using PrivStack.Services;
using PrivStack.Services.Plugin;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using Serilog;

namespace PrivStack.Services.Sdk;

/// <summary>
/// Implements IPluginHost by composing the SdkHost, CapabilityBroker, and other services.
/// One instance per plugin, with plugin-scoped settings and logger.
/// </summary>
internal sealed class PluginHost : IPluginHost
{
    public PluginHost(
        IPrivStackSdk sdk,
        ICapabilityBroker capabilities,
        string pluginId,
        ISdkDialogService dialogService,
        Services.Abstractions.IAppSettingsService appSettings,
        IPluginRegistry pluginRegistry,
        Services.Abstractions.IUiDispatcher dispatcher,
        IInfoPanelService infoPanelService,
        IFocusModeService focusModeService,
        IToastService toastService,
        IConnectionService? connectionService = null,
        IPropertyService? propertyService = null,
        IAiService? aiService = null,
        IIntentEngine? intentEngine = null,
        IAiSuggestionService? suggestionService = null,
        IAudioRecorderService? audioRecorder = null,
        ITranscriptionService? transcription = null)
    {
        Sdk = new TrackedSdkProxy(sdk, $"plugin.{pluginId}");
        Capabilities = capabilities;
        Settings = new PluginSettingsAdapter(pluginId, appSettings);
        Logger = new SerilogPluginLogger(pluginId);
        Navigation = new NavigationServiceAdapter(pluginRegistry, dispatcher);
        DialogService = dialogService;
        InfoPanel = infoPanelService;
        FocusMode = focusModeService;
        Toast = toastService;
        Connections = connectionService;
        Properties = propertyService;
        AI = aiService;
        IntentEngine = intentEngine;
        Suggestions = suggestionService;
        AudioRecorder = audioRecorder;
        Transcription = transcription;
        Messenger = WeakReferenceMessenger.Default;
        AppVersion = typeof(PluginHost).Assembly.GetName().Version ?? new Version(1, 0, 0);
    }

    public IPrivStackSdk Sdk { get; }
    public ICapabilityBroker Capabilities { get; }
    public IPluginSettings Settings { get; }
    public IPluginLogger Logger { get; }
    public INavigationService Navigation { get; }
    public ISdkDialogService? DialogService { get; }
    public IInfoPanelService InfoPanel { get; }
    public IFocusModeService FocusMode { get; }
    public IToastService Toast { get; }
    public IConnectionService? Connections { get; }
    public IPropertyService? Properties { get; }
    public IAiService? AI { get; }
    public IIntentEngine? IntentEngine { get; }
    public IAiSuggestionService? Suggestions { get; }
    public IAudioRecorderService? AudioRecorder { get; }
    public ITranscriptionService? Transcription { get; }
    public IMessenger Messenger { get; }
    public Version AppVersion { get; }

    public string WorkspaceDataPath =>
        Services.DataPaths.WorkspaceDataDir
        ?? throw new InvalidOperationException(
            "No active workspace. Plugins cannot access WorkspaceDataPath before workspace selection.");
}

/// <summary>
/// Plugin-namespaced settings backed by AppSettingsService.
/// Keys are stored as "plugin.{id}.{key}" in the flat PluginSettings dictionary.
/// </summary>
internal sealed class PluginSettingsAdapter : IPluginSettings
{
    private readonly string _prefix;
    private readonly Services.Abstractions.IAppSettingsService _appSettings;

    public PluginSettingsAdapter(string pluginId, Services.Abstractions.IAppSettingsService appSettings)
    {
        _prefix = $"plugin.{pluginId}.";
        _appSettings = appSettings;
    }

    public T Get<T>(string key, T defaultValue)
    {
        var fullKey = _prefix + key;
        var dict = _appSettings.Settings.PluginSettings;

        if (!dict.TryGetValue(fullKey, out var json))
            return defaultValue;

        try
        {
            return JsonSerializer.Deserialize<T>(json) ?? defaultValue;
        }
        catch
        {
            return defaultValue;
        }
    }

    public void Set<T>(string key, T value)
    {
        var fullKey = _prefix + key;
        var json = JsonSerializer.Serialize(value);
        _appSettings.Settings.PluginSettings[fullKey] = json;
        _appSettings.SaveDebounced();
    }
}

/// <summary>
/// Wraps Serilog with plugin ID in context.
/// </summary>
internal sealed class SerilogPluginLogger : IPluginLogger
{
    private readonly ILogger _log;

    public SerilogPluginLogger(string pluginId)
    {
        _log = Serilog.Log.ForContext("PluginId", pluginId);
    }

    public void Debug(string messageTemplate, params object[] args) => _log.Debug(messageTemplate, args);
    public void Info(string messageTemplate, params object[] args) => _log.Information(messageTemplate, args);
    public void Warn(string messageTemplate, params object[] args) => _log.Warning(messageTemplate, args);
    public void Error(string messageTemplate, params object[] args) => _log.Error(messageTemplate, args);
    public void Error(Exception ex, string messageTemplate, params object[] args) => _log.Error(ex, messageTemplate, args);
}

/// <summary>
/// Cross-plugin navigation wired to MainWindowViewModel via PluginRegistry.
/// </summary>
internal sealed class NavigationServiceAdapter : INavigationService
{
    private static string? _previousNavItemId;
    private readonly IPluginRegistry _pluginRegistry;
    private readonly Services.Abstractions.IUiDispatcher _dispatcher;

    public NavigationServiceAdapter(IPluginRegistry pluginRegistry, Services.Abstractions.IUiDispatcher dispatcher)
    {
        _pluginRegistry = pluginRegistry;
        _dispatcher = dispatcher;
    }

    public void NavigateTo(string pluginId)
    {
        var plugin = _pluginRegistry.GetPlugin(pluginId);
        var navItemId = plugin?.NavigationItem?.Id;
        if (navItemId == null) return;

        NavigateToNavItem(navItemId);
    }

    public void NavigateBack()
    {
        if (_previousNavItemId != null)
            NavigateToNavItem(_previousNavItemId);
    }

    public Task NavigateToItemAsync(string linkType, string itemId)
    {
        var host = _pluginRegistry.GetMainViewModel() as Abstractions.INavigationHost;
        if (host == null) return Task.CompletedTask;
        return _dispatcher.InvokeAsync(() => host.NavigateToLinkedItemAsync(linkType, itemId));
    }

    private void NavigateToNavItem(string navItemId)
    {
        var host = _pluginRegistry.GetMainViewModel() as Abstractions.INavigationHost;
        if (host == null) return;

        _previousNavItemId = host.SelectedTab;

        _dispatcher.Post(() => host.SelectTab(navItemId));
    }
}

/// <summary>
/// Wraps IPrivStackSdk to tag SendAsync calls with a subsystem scope,
/// so plugin SDK activity shows in the Subsystems tab.
/// </summary>
internal sealed class TrackedSdkProxy(IPrivStackSdk inner, string subsystemId) : IPrivStackSdk
{
    public bool IsReady => inner.IsReady;

    public Task<SdkResponse<TResult>> SendAsync<TResult>(SdkMessage message, CancellationToken ct = default)
    {
        using var _ = Diagnostics.SubsystemTracker.Instance?.EnterScope(subsystemId);
        return inner.SendAsync<TResult>(message, ct);
    }

    public Task<SdkResponse> SendAsync(SdkMessage message, CancellationToken ct = default)
    {
        using var _ = Diagnostics.SubsystemTracker.Instance?.EnterScope(subsystemId);
        return inner.SendAsync(message, ct);
    }

    public Task<int> CountAsync(string pluginId, string entityType, bool includeTrashed = false, CancellationToken ct = default)
    {
        using var _ = Diagnostics.SubsystemTracker.Instance?.EnterScope(subsystemId);
        return inner.CountAsync(pluginId, entityType, includeTrashed, ct);
    }

    public Task<SdkResponse<TResult>> SearchAsync<TResult>(string query, string[]? entityTypes = null, int limit = 50, CancellationToken ct = default)
    {
        using var _ = Diagnostics.SubsystemTracker.Instance?.EnterScope(subsystemId);
        return inner.SearchAsync<TResult>(query, entityTypes, limit, ct);
    }

    // Pass-through for non-data operations (no tracking needed)
    public Task RunDatabaseMaintenance(CancellationToken ct = default) => inner.RunDatabaseMaintenance(ct);
    public string GetDatabaseDiagnostics() => inner.GetDatabaseDiagnostics();
    public string FindOrphanEntities(string validTypesJson) => inner.FindOrphanEntities(validTypesJson);
    public string DeleteOrphanEntities(string validTypesJson) => inner.DeleteOrphanEntities(validTypesJson);
    public string CompactDatabases() => inner.CompactDatabases();

    public Task<bool> VaultIsInitialized(string vaultId, CancellationToken ct = default) => inner.VaultIsInitialized(vaultId, ct);
    public Task VaultInitialize(string vaultId, string password, CancellationToken ct = default) => inner.VaultInitialize(vaultId, password, ct);
    public Task VaultUnlock(string vaultId, string password, CancellationToken ct = default) => inner.VaultUnlock(vaultId, password, ct);
    public Task VaultLock(string vaultId, CancellationToken ct = default) => inner.VaultLock(vaultId, ct);
    public Task<bool> VaultIsUnlocked(string vaultId, CancellationToken ct = default) => inner.VaultIsUnlocked(vaultId, ct);
    public Task<bool> RequestVaultUnlockAsync(string vaultId, CancellationToken ct = default) => inner.RequestVaultUnlockAsync(vaultId, ct);

    public Task VaultBlobStore(string vaultId, string blobId, byte[] data, CancellationToken ct = default) => inner.VaultBlobStore(vaultId, blobId, data, ct);
    public Task<byte[]> VaultBlobRead(string vaultId, string blobId, CancellationToken ct = default) => inner.VaultBlobRead(vaultId, blobId, ct);
    public Task VaultBlobDelete(string vaultId, string blobId, CancellationToken ct = default) => inner.VaultBlobDelete(vaultId, blobId, ct);

    public Task BlobStore(string ns, string blobId, byte[] data, string? metadataJson = null, CancellationToken ct = default) => inner.BlobStore(ns, blobId, data, metadataJson, ct);
    public Task<byte[]> BlobRead(string ns, string blobId, CancellationToken ct = default) => inner.BlobRead(ns, blobId, ct);
    public Task BlobDelete(string ns, string blobId, CancellationToken ct = default) => inner.BlobDelete(ns, blobId, ct);
}
