using System.Text.Json;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Helpers;
using Serilog;

namespace PrivStack.Desktop.Services;

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
        EntityMetadataService entityMetadata)
    {
        _sdk = sdk;
        _appSettings = appSettings;
        _pluginRegistry = pluginRegistry;
        _entityMetadata = entityMetadata;
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

        // 1. Wipe system metadata
        foreach (var (pluginId, entityType) in SystemWipeTargets)
            await SeedHelper.DeleteAllEntitiesAsync(_sdk, pluginId, entityType);

        // 2. Wipe all plugin data via ISeedDataProvider
        var providers = _pluginRegistry.GetCapabilityProviders<ISeedDataProvider>();
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

        // 1. Wipe system metadata
        foreach (var (pluginId, entityType) in SystemWipeTargets)
            await SeedHelper.DeleteAllEntitiesAsync(_sdk, pluginId, entityType);

        // 2. Wipe all plugin data via ISeedDataProvider
        var providers = _pluginRegistry.GetCapabilityProviders<ISeedDataProvider>();
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

        _appSettings.Settings.SampleDataSeeded = false;
        _appSettings.Settings.SeedDataVersion = 0;
        _appSettings.Save();

        _entityMetadata.InvalidateAll();
        _log.Information("All plugin data wiped, reseeding...");

        // 3. Seed all plugin data
        await SeedAllPluginsAsync();

        _appSettings.Settings.SampleDataSeeded = true;
        _appSettings.Save();
        _log.Information("Sample data reseeded successfully");
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
}
