// ============================================================================
// File: JsonElementExtensions.cs
// Description: Safe extraction helpers for System.Text.Json elements.
// ============================================================================

using System.Text.Json;

namespace PrivStack.UI.Adaptive;

/// <summary>
/// Extension methods for safe property extraction from <see cref="JsonElement"/>.
/// </summary>
public static class JsonElementExtensions
{
    public static string? GetStringProp(this JsonElement el, string name)
    {
        if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.String)
            return prop.GetString();
        return null;
    }

    public static int GetIntProp(this JsonElement el, string name, int defaultValue = 0)
    {
        if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.Number)
            return prop.GetInt32();
        return defaultValue;
    }

    public static long GetInt64Prop(this JsonElement el, string name, long defaultValue = 0)
    {
        if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.Number)
            return prop.GetInt64();
        return defaultValue;
    }

    public static double GetDoubleProp(this JsonElement el, string name, double defaultValue = 0)
    {
        if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.Number)
            return prop.GetDouble();
        return defaultValue;
    }

    public static bool GetBoolProp(this JsonElement el, string name, bool defaultValue = false)
    {
        if (el.TryGetProperty(name, out var prop))
        {
            if (prop.ValueKind == JsonValueKind.True) return true;
            if (prop.ValueKind == JsonValueKind.False) return false;
            // Truthy: non-zero numbers
            if (prop.ValueKind == JsonValueKind.Number) return prop.GetDouble() != 0;
        }
        return defaultValue;
    }
}
