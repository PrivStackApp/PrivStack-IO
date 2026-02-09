// ============================================================================
// File: TemplateFilters.cs
// Description: Built-in filters and truthiness evaluation for the template
//              engine. Filters transform JsonNode values using a pipe syntax:
//              {{value | filter: arg}}
// ============================================================================

using System.Text.Json;
using System.Text.Json.Nodes;
using Serilog;

namespace PrivStack.UI.Adaptive;

/// <summary>
/// Evaluates built-in template filters and truthiness for control flow.
/// </summary>
internal static class TemplateFilters
{
    private static readonly ILogger Log = Serilog.Log.ForContext(typeof(TemplateFilters));

    /// <summary>
    /// Determines if a JSON value is "truthy" for $if evaluation.
    /// Falsy: null, false, 0, "", empty array, empty object.
    /// </summary>
    public static bool IsTruthy(JsonNode? value)
    {
        if (value is null) return false;

        return value switch
        {
            JsonValue jv when jv.TryGetValue(out bool b) => b,
            JsonValue jv when jv.TryGetValue(out int i) => i != 0,
            JsonValue jv when jv.TryGetValue(out long l) => l != 0,
            JsonValue jv when jv.TryGetValue(out double d) => d != 0.0,
            JsonValue jv when jv.TryGetValue(out string? s) => !string.IsNullOrEmpty(s),
            JsonArray ja => ja.Count > 0,
            JsonObject jo => jo.Count > 0,
            _ => true,
        };
    }

    /// <summary>
    /// Applies a named filter to an input value with an optional argument.
    /// Unknown filters pass the value through unchanged with a warning.
    /// </summary>
    public static JsonNode? Apply(string filterName, JsonNode? input, string? arg)
    {
        return filterName switch
        {
            "default" => input is null || !IsTruthy(input)
                ? (arg is not null ? JsonValue.Create(arg) : null)
                : input,

            "size" => input switch
            {
                JsonArray ja => JsonValue.Create(ja.Count),
                JsonValue jv when jv.TryGetValue(out string? s) => JsonValue.Create(s?.Length ?? 0),
                JsonObject jo => JsonValue.Create(jo.Count),
                _ => JsonValue.Create(0),
            },

            "truncate" => ApplyTruncate(input, arg),

            "escape" => input switch
            {
                JsonValue jv when jv.TryGetValue(out string? s) =>
                    JsonValue.Create(System.Net.WebUtility.HtmlEncode(s ?? "")),
                _ => input,
            },

            "upcase" => input switch
            {
                JsonValue jv when jv.TryGetValue(out string? s) =>
                    JsonValue.Create(s?.ToUpperInvariant() ?? ""),
                _ => input,
            },

            "downcase" => input switch
            {
                JsonValue jv when jv.TryGetValue(out string? s) =>
                    JsonValue.Create(s?.ToLowerInvariant() ?? ""),
                _ => input,
            },

            "json" => input is not null
                ? JsonValue.Create(input.ToJsonString())
                : JsonValue.Create("null"),

            "not" or "negate" => JsonValue.Create(!IsTruthy(input)),

            "if_true" => IsTruthy(input) && arg is not null
                ? ParseIfTrueArg(arg)
                : input,

            "duration" => ApplyDuration(input),

            "first" => input is JsonArray { Count: > 0 } ja
                ? ja[0]?.DeepClone()
                : null,

            "last" => input is JsonArray { Count: > 0 } ja
                ? ja[ja.Count - 1]?.DeepClone()
                : null,

            "join" => input is JsonArray ja
                ? JsonValue.Create(string.Join(
                    arg ?? ", ",
                    ja.Select(n => NodeToString(n))))
                : input,

            "append" => input switch
            {
                JsonValue jv when jv.TryGetValue(out string? s) =>
                    JsonValue.Create((s ?? "") + (arg ?? "")),
                _ => input is not null
                    ? JsonValue.Create(NodeToString(input) + (arg ?? ""))
                    : (arg is not null ? JsonValue.Create(arg) : null),
            },

            "prepend" => input switch
            {
                JsonValue jv when jv.TryGetValue(out string? s) =>
                    JsonValue.Create((arg ?? "") + (s ?? "")),
                _ => input is not null
                    ? JsonValue.Create((arg ?? "") + NodeToString(input))
                    : (arg is not null ? JsonValue.Create(arg) : null),
            },

            "date" => ApplyDate(input, arg),

            _ => LogUnknownFilter(filterName, input),
        };
    }

    private static JsonNode? ApplyTruncate(JsonNode? input, string? arg)
    {
        if (input is not JsonValue jv || !jv.TryGetValue(out string? s) || s is null)
            return input;

        if (!int.TryParse(arg, out var maxLen) || maxLen <= 0)
            return input;

        return s.Length <= maxLen
            ? input
            : JsonValue.Create(string.Concat(s.AsSpan(0, maxLen), "..."));
    }

    private static JsonNode? ApplyDate(JsonNode? input, string? format)
    {
        if (input is not JsonValue jv) return input;

        DateTimeOffset dto;

        // Handle numeric timestamps (auto-detect seconds vs milliseconds)
        // Values > 10_000_000_000 are treated as milliseconds
        if (jv.TryGetValue(out long epochLong))
        {
            dto = epochLong > 10_000_000_000L
                ? DateTimeOffset.FromUnixTimeMilliseconds(epochLong)
                : DateTimeOffset.FromUnixTimeSeconds(epochLong);
        }
        else if (jv.TryGetValue(out int epochInt))
        {
            dto = DateTimeOffset.FromUnixTimeSeconds(epochInt);
        }
        else if (jv.TryGetValue(out double epochDbl))
        {
            var epochL = (long)epochDbl;
            dto = epochL > 10_000_000_000L
                ? DateTimeOffset.FromUnixTimeMilliseconds(epochL)
                : DateTimeOffset.FromUnixTimeSeconds(epochL);
        }
        else if (jv.TryGetValue(out string? str) && !string.IsNullOrEmpty(str))
        {
            if (!DateTimeOffset.TryParse(str, System.Globalization.CultureInfo.InvariantCulture,
                    System.Globalization.DateTimeStyles.None, out dto))
            {
                if (long.TryParse(str, out var epoch))
                    dto = DateTimeOffset.FromUnixTimeSeconds(epoch);
                else
                    return input;
            }
        }
        else
        {
            return input;
        }

        // Convert strftime-style format to .NET
        var fmt = (format ?? "%b %d, %Y")
            .Replace("%Y", "yyyy")
            .Replace("%m", "MM")
            .Replace("%d", "dd")
            .Replace("%H", "HH")
            .Replace("%I", "hh")
            .Replace("%M", "mm")
            .Replace("%S", "ss")
            .Replace("%p", "tt")
            .Replace("%b", "MMM")
            .Replace("%B", "MMMM")
            .Replace("%a", "ddd")
            .Replace("%A", "dddd")
            .Replace("%Z", "zzz");

        return JsonValue.Create(dto.ToLocalTime().ToString(fmt.Trim('"', ' ')));
    }

    /// <summary>
    /// Parses "if_true" filter arg: "result | default: fallback" → result if truthy, fallback otherwise.
    /// Usage: {{value | if_true: danger | default: default}}
    /// The template engine chains filters, so we only handle the immediate arg here.
    /// </summary>
    private static JsonNode? ParseIfTrueArg(string arg)
    {
        // arg is the value after "if_true:", e.g. "danger"
        return JsonValue.Create(arg.Trim());
    }

    /// <summary>
    /// Formats a millisecond duration into HH:MM:SS.
    /// </summary>
    private static JsonNode? ApplyDuration(JsonNode? input)
    {
        if (input is not JsonValue jv) return input;

        long ms = 0;
        if (jv.TryGetValue(out long l)) ms = l;
        else if (jv.TryGetValue(out int i)) ms = i;
        else if (jv.TryGetValue(out double d)) ms = (long)d;
        else return input;

        var totalSeconds = ms / 1000;
        var hours = totalSeconds / 3600;
        var minutes = (totalSeconds % 3600) / 60;
        var seconds = totalSeconds % 60;
        return JsonValue.Create($"{hours:D2}:{minutes:D2}:{seconds:D2}");
    }

    private static JsonNode? LogUnknownFilter(string filterName, JsonNode? input)
    {
        Log.Warning("Unknown template filter: {FilterName}", filterName);
        return input;
    }

    /// <summary>
    /// Converts a JsonNode to its string representation for interpolation.
    /// </summary>
    internal static string NodeToString(JsonNode? node)
    {
        if (node is null) return "";
        if (node is JsonValue jv)
        {
            if (jv.TryGetValue(out string? s)) return s ?? "";
            if (jv.TryGetValue(out bool b)) return b ? "true" : "false";
            if (jv.TryGetValue(out int i)) return i.ToString();
            if (jv.TryGetValue(out long l)) return l.ToString();
            if (jv.TryGetValue(out double d)) return d.ToString();
        }
        return node.ToJsonString();
    }
}
