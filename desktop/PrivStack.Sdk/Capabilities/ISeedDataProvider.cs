namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that provide seed (demo) data.
/// Discovered via IPluginRegistry.GetCapabilityProviders&lt;ISeedDataProvider&gt;().
/// </summary>
public interface ISeedDataProvider
{
    /// <summary>
    /// Entity types owned by this plugin, in deletion order (children before parents).
    /// Each tuple is (pluginId, entityType).
    /// </summary>
    IReadOnlyList<(string PluginId, string EntityType)> WipeTargets { get; }

    /// <summary>
    /// Seeds demo data. Called on first run or manual reseed.
    /// </summary>
    Task SeedAsync(CancellationToken ct = default);

    /// <summary>
    /// Deletes all entities for this plugin's entity types.
    /// </summary>
    Task WipeAsync(IPrivStackSdk sdk, CancellationToken ct = default);
}
