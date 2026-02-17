namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that require encrypted vault storage.
/// Plugins declare their vault IDs so the host can discover, unlock,
/// and manage them without hardcoding vault identifiers.
/// </summary>
public interface IVaultConsumer
{
    /// <summary>
    /// Vault identifiers used by this plugin for encrypted credential/blob storage.
    /// </summary>
    IReadOnlyList<string> VaultIds { get; }
}
