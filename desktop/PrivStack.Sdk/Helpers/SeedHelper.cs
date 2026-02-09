using System.Text.Json;
using PrivStack.Sdk.Json;
using Serilog;

namespace PrivStack.Sdk.Helpers;

/// <summary>
/// Shared helper methods for plugin seed services.
/// Encapsulates the low-level SDK calls for creating and deleting entities.
/// </summary>
public static class SeedHelper
{
    private static readonly ILogger Log = Serilog.Log.ForContext(typeof(SeedHelper));
    private static readonly JsonSerializerOptions Json = SdkJsonOptions.Default;

    /// <summary>
    /// Deletes all entities of a given type for a plugin (including trashed items).
    /// </summary>
    public static async Task DeleteAllEntitiesAsync(IPrivStackSdk sdk, string pluginId, string entityType)
    {
        try
        {
            var response = await sdk.SendAsync<List<JsonElement>>(new SdkMessage
            {
                PluginId = pluginId,
                Action = SdkAction.ReadList,
                EntityType = entityType,
                Parameters = new Dictionary<string, string>
                {
                    ["include_trashed"] = "true",
                },
            });

            var items = response.Data;
            if (items == null || items.Count == 0) return;

            Log.Debug("Deleting {Count} {EntityType} entities from {PluginId}", items.Count, entityType, pluginId);

            foreach (var item in items)
            {
                var id = item.GetProperty("id").GetString();
                if (string.IsNullOrEmpty(id)) continue;

                await sdk.SendAsync(new SdkMessage
                {
                    PluginId = pluginId,
                    Action = SdkAction.Delete,
                    EntityType = entityType,
                    EntityId = id,
                });
            }
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to delete {EntityType} entities for {PluginId}", entityType, pluginId);
        }
    }

    /// <summary>
    /// Creates an entity via the SDK and returns the assigned ID.
    /// </summary>
    public static async Task<string> CreateEntityAsync(
        IPrivStackSdk sdk, string pluginId, string entityType, object data,
        DateTimeOffset? createdAt = null, DateTimeOffset? modifiedAt = null)
    {
        var payload = JsonSerializer.Serialize(data, Json);
        var response = await sdk.SendAsync<JsonElement>(new SdkMessage
        {
            PluginId = pluginId,
            Action = SdkAction.Create,
            EntityType = entityType,
            Payload = payload,
            Parameters = BuildDateParameters(createdAt, modifiedAt),
        });

        return response.Data.GetProperty("id").GetString()!;
    }

    /// <summary>
    /// Updates an existing entity via the SDK.
    /// </summary>
    public static async Task UpdateEntityAsync(
        IPrivStackSdk sdk, string pluginId, string entityType, string entityId, object data,
        DateTimeOffset? createdAt = null, DateTimeOffset? modifiedAt = null)
    {
        var payload = JsonSerializer.Serialize(data, Json);
        await sdk.SendAsync(new SdkMessage
        {
            PluginId = pluginId,
            Action = SdkAction.Update,
            EntityType = entityType,
            EntityId = entityId,
            Payload = payload,
            Parameters = BuildDateParameters(createdAt, modifiedAt),
        });
    }

    /// <summary>
    /// Sends a command to the SDK.
    /// </summary>
    public static async Task SendCommandAsync(
        IPrivStackSdk sdk, string pluginId, string entityType, string entityId,
        Dictionary<string, string> parameters)
    {
        try
        {
            await sdk.SendAsync(new SdkMessage
            {
                PluginId = pluginId,
                Action = SdkAction.Command,
                EntityType = entityType,
                EntityId = entityId,
                Parameters = parameters,
            });
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Command failed for {PluginId}/{EntityType}/{EntityId}", pluginId, entityType, entityId);
        }
    }

    /// <summary>
    /// Reads an entity and returns the raw JSON.
    /// </summary>
    public static async Task<JsonElement?> ReadEntityAsync(
        IPrivStackSdk sdk, string pluginId, string entityType, string entityId)
    {
        try
        {
            var response = await sdk.SendAsync<JsonElement>(new SdkMessage
            {
                PluginId = pluginId,
                Action = SdkAction.Read,
                EntityType = entityType,
                EntityId = entityId,
            });

            return response.Success ? response.Data : null;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>
    /// Builds date parameter dictionary for SDK messages.
    /// </summary>
    public static Dictionary<string, string>? BuildDateParameters(
        DateTimeOffset? createdAt, DateTimeOffset? modifiedAt)
    {
        if (createdAt == null && modifiedAt == null) return null;
        var p = new Dictionary<string, string>();
        if (createdAt.HasValue)
            p["created_at"] = createdAt.Value.ToUnixTimeMilliseconds().ToString();
        if (modifiedAt.HasValue)
            p["modified_at"] = modifiedAt.Value.ToUnixTimeMilliseconds().ToString();
        return p;
    }
}
