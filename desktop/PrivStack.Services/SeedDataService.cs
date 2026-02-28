using System.Text.Json;
using PrivStack.Services.Native;
using PrivStack.Services.Abstractions;
using PrivStack.Services.AI;
using PrivStack.Services.Connections;
using PrivStack.Services.Plugin;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Helpers;
using Serilog;
using NativeLib = PrivStack.Services.Native.NativeLibrary;

namespace PrivStack.Services;

/// <summary>
/// Thin orchestrator that delegates seed/wipe operations to individual plugin ISeedDataProvider implementations.
/// The host only manages system metadata wipe and the seeded-flag bookkeeping.
/// </summary>
public sealed class SeedDataService
{
    private static readonly ILogger _log = Log.ForContext<SeedDataService>();

    private readonly IPrivStackSdk _sdk;
    private readonly IAppSettingsService _appSettings;
    private readonly IPluginRegistry _pluginRegistry;
    private readonly EntityMetadataService _entityMetadata;
    private readonly ConnectionService _connectionService;
    private readonly AiConversationStore _conversationStore;
    private readonly AiMemoryService _memoryService;
    private readonly ICloudSyncService _cloudSync;
    private readonly IWorkspaceService _workspaceService;
    private readonly PrivStackApiClient _apiClient;

    /// <summary>
    /// System-level entity types that the host owns (not any plugin).
    /// Wiped in this order before plugin data.
    /// </summary>
    private static readonly (string PluginId, string EntityType)[] SystemWipeTargets =
    [
        ("privstack.system", "entity_metadata"),
        ("privstack.system", "property_template"),
        ("privstack.system", "property_definition"),
        ("privstack.system", "property_group"),
    ];

    public SeedDataService(
        IPrivStackSdk sdk,
        IAppSettingsService appSettings,
        IPluginRegistry pluginRegistry,
        EntityMetadataService entityMetadata,
        ConnectionService connectionService,
        AiConversationStore conversationStore,
        AiMemoryService memoryService,
        ICloudSyncService cloudSync,
        IWorkspaceService workspaceService,
        PrivStackApiClient apiClient)
    {
        _sdk = sdk;
        _appSettings = appSettings;
        _pluginRegistry = pluginRegistry;
        _entityMetadata = entityMetadata;
        _connectionService = connectionService;
        _conversationStore = conversationStore;
        _memoryService = memoryService;
        _cloudSync = cloudSync;
        _workspaceService = workspaceService;
        _apiClient = apiClient;
    }

    /// <summary>
    /// Seeds sample data if it hasn't been seeded yet.
    /// </summary>
    public async Task SeedIfNeededAsync()
    {
        if (_appSettings.Settings.SampleDataSeeded)
        {
            _log.Debug("Sample data already seeded, skipping");
            return;
        }

        if (!_sdk.IsReady)
        {
            _log.Warning("SDK not ready, skipping sample data seed");
            return;
        }

        try
        {
            _log.Information("Seeding sample data...");
            await SeedAllPluginsAsync();
            _appSettings.Settings.SampleDataSeeded = true;
            _appSettings.Save();
            _log.Information("Sample data seeded successfully");
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to seed sample data");
        }
    }

    /// <summary>
    /// Wipes all seeded plugin data without reseeding. Resets the DB to a clean state.
    /// </summary>
    public async Task WipeAsync()
    {
        _log.Information("Wiping all seeded data...");

        await WipeAllPluginDataAsync();

        _appSettings.Settings.SampleDataSeeded = false;
        _appSettings.Settings.SeedDataVersion = 0;
        _appSettings.Save();

        _entityMetadata.InvalidateAll();
        _log.Information("All seeded data wiped successfully");
    }

    /// <summary>
    /// Wipes all plugin data and reseeds with fresh sample data.
    /// </summary>
    public async Task ReseedAsync()
    {
        _log.Information("Reseeding sample data — wiping all plugin data...");

        await WipeAllPluginDataAsync();

        _appSettings.Settings.SampleDataSeeded = false;
        _appSettings.Settings.SeedDataVersion = 0;
        _appSettings.Save();

        _entityMetadata.InvalidateAll();
        _log.Information("All plugin data wiped, reseeding...");

        // Seed all plugin data
        await SeedAllPluginsAsync();

        _appSettings.Settings.SampleDataSeeded = true;
        _appSettings.Save();
        _log.Information("Sample data reseeded successfully");

        // Kick off full RAG re-index in the background now that fresh data exists
        TriggerFullRagIndex();
    }

    /// <summary>
    /// Wipes system metadata, then each plugin's declared WipeTargets (entity types),
    /// then calls each plugin's WipeAsync for non-entity cleanup (e.g. DuckDB tables).
    /// </summary>
    private async Task WipeAllPluginDataAsync()
    {
        // 0. Purge cloud workspace data (batches, cursors, snapshots, blobs, S3 objects)
        //    so the server doesn't report stale batches as "pending" after local wipe.
        await PurgeCloudWorkspaceIfConnectedAsync();

        // 1. Wipe system metadata
        foreach (var (pluginId, entityType) in SystemWipeTargets)
            await SeedHelper.DeleteAllEntitiesAsync(_sdk, pluginId, entityType);

        // 2. Delete all entity types declared in each provider's WipeTargets.
        //    This ensures entity-based data is cleaned up even if a plugin's
        //    WipeAsync implementation is incomplete.
        //    Use Plugins (all) not GetCapabilityProviders (active-only) so disabled plugins are wiped too.
        var providers = _pluginRegistry.Plugins.OfType<ISeedDataProvider>().ToList();
        foreach (var provider in providers)
        {
            foreach (var (pluginId, entityType) in provider.WipeTargets)
            {
                try
                {
                    await SeedHelper.DeleteAllEntitiesAsync(_sdk, pluginId, entityType);
                }
                catch (Exception ex)
                {
                    _log.Warning(ex, "Failed to delete {EntityType} entities for {PluginId}",
                        entityType, pluginId);
                }
            }
        }

        // 3. Call each provider's WipeAsync for custom cleanup (DuckDB tables, files, etc.)
        foreach (var provider in providers)
        {
            try
            {
                await provider.WipeAsync(_sdk);
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to wipe data for seed provider");
            }
        }

        // 4. Clear AI state (RAG vectors, conversation history, memories)
        ClearAiState();

        // 5. Disconnect all external service connections (GitHub, etc.)
        await DisconnectAllConnectionsAsync();
    }

    /// <summary>
    /// Clears all AI-related state: RAG vectors, conversation history, and learned memories.
    /// </summary>
    private void ClearAiState()
    {
        try
        {
            // Clear RAG vector index
            var resultPtr = NativeLib.RagDeleteAll();
            if (resultPtr != nint.Zero)
                NativeLib.FreeString(resultPtr);
            _log.Information("RAG vector index cleared");
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to clear RAG vectors during wipe");
        }

        try
        {
            _conversationStore.ClearAll();
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to clear AI conversations during wipe");
        }

        try
        {
            _memoryService.ClearDataMemories();
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to clear data AI memories during wipe");
        }
    }

    /// <summary>
    /// Disconnects all external service connections (deletes vault tokens and metadata).
    /// </summary>
    private async Task DisconnectAllConnectionsAsync()
    {
        var providers = _appSettings.Settings.ConnectionMetadata.Keys.ToList();
        foreach (var provider in providers)
        {
            try
            {
                await _connectionService.DisconnectAsync(provider);
                _log.Information("Disconnected {Provider} during data wipe", provider);
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to disconnect {Provider} during wipe", provider);
            }
        }
    }

    /// <summary>
    /// If cloud sync is active, stops the sync engine and purges all server-side data
    /// (batches, cursors, snapshots, blobs, S3 objects) so stale batches aren't
    /// re-downloaded after the local wipe completes.
    /// </summary>
    private async Task PurgeCloudWorkspaceIfConnectedAsync()
    {
        try
        {
            var workspace = _workspaceService.GetActiveWorkspace();
            var token = _appSettings.Settings.CloudSyncAccessToken;

            if (workspace?.CloudWorkspaceId == null || string.IsNullOrEmpty(token))
                return;

            if (_cloudSync.IsSyncing)
                _cloudSync.StopSync();

            var result = await _apiClient.PurgeCloudWorkspaceAsync(token, workspace.CloudWorkspaceId);
            _log.Information(
                "Cloud workspace purged during data wipe: {DeletedObjects} objects, {FreedBytes} bytes freed",
                result.DeletedObjects, result.FreedBytes);

            // Clear local cursor state so stale cursors don't cause phantom
            // re-uploads when sync restarts after the wipe.
            _cloudSync.ClearCursors();
            _log.Debug("Cleared local cloud sync cursors after purge");
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "Failed to purge cloud workspace during data wipe (continuing local wipe)");
        }
    }

    private async Task SeedAllPluginsAsync()
    {
        var providers = _pluginRegistry.GetCapabilityProviders<ISeedDataProvider>();
        _log.Debug("Found {Count} seed data providers", providers.Count);

        foreach (var provider in providers)
        {
            try
            {
                await provider.SeedAsync();
            }
            catch (Exception ex)
            {
                _log.Warning(ex, "Failed to seed data for provider");
            }
        }
    }

    private void TriggerFullRagIndex()
    {
        try
        {
            var ragIndexService = Microsoft.Extensions.DependencyInjection.ServiceProviderServiceExtensions
                .GetRequiredService<AI.RagIndexService>(ServiceProviderAccessor.Services);
            _ = Task.Run(() => ragIndexService.StartFullIndexAsync());
        }
        catch (Exception ex)
        {
            _log.Debug(ex, "RAG re-index after reseed skipped (service not available)");
        }
    }
}
