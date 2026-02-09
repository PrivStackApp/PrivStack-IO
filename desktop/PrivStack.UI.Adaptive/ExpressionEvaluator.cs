// ============================================================================
// File: ExpressionEvaluator.cs
// Description: Evaluates {{path.to.value | filter: arg}} expressions against
//              a scoped JSON data model. Single-expression strings resolve to
//              native JSON types; mixed text produces string interpolation.
// ============================================================================

using System.Text;
using System.Text.Json.Nodes;
using Serilog;

namespace PrivStack.UI.Adaptive;

/// <summary>
/// Scoped evaluation context for template expressions.
/// Resolves paths against a stack of scopes (innermost first), then the root data.
/// </summary>
internal sealed class EvaluationContext
{
    private readonly JsonObject _root;
    private readonly Stack<Dictionary<string, JsonNode?>> _scopes = new();

    public EvaluationContext(JsonObject root)
    {
        _root = root;
    }

    public void PushScope(Dictionary<string, JsonNode?> scope) => _scopes.Push(scope);
    public void PopScope() => _scopes.Pop();

    /// <summary>
    /// Resolves a dot-path like "feed.title" or "loop.index" against scopes then root.
    /// </summary>
    public JsonNode? Resolve(string dotPath)
    {
        if (string.IsNullOrEmpty(dotPath)) return null;

        var segments = dotPath.Split('.');
        var firstSegment = segments[0];

        // Check scopes (innermost first)
        foreach (var scope in _scopes)
        {
            if (scope.TryGetValue(firstSegment, out var scopeVal))
            {
                return WalkPath(scopeVal, segments, 1);
            }
        }

        // Fall back to root
        if (_root.TryGetPropertyValue(firstSegment, out var rootVal))
        {
            return WalkPath(rootVal, segments, 1);
        }

        return null;
    }

    private static JsonNode? WalkPath(JsonNode? node, string[] segments, int startIndex)
    {
        for (int i = startIndex; i < segments.Length; i++)
        {
            if (node is null) return null;

            if (node is JsonObject obj)
            {
                if (!obj.TryGetPropertyValue(segments[i], out node))
                    return null;
            }
            else if (node is JsonArray arr && int.TryParse(segments[i], out var idx))
            {
                node = idx >= 0 && idx < arr.Count ? arr[idx] : null;
            }
            else
            {
                return null;
            }
        }
        return node;
    }
}

/// <summary>
/// Evaluates template expression strings containing {{...}} placeholders.
/// </summary>
internal static class ExpressionEvaluator
{
    private static readonly ILogger Log = Serilog.Log.ForContext(typeof(ExpressionEvaluator));

    /// <summary>
    /// Evaluates a template string. If it is a single pure expression like "{{feeds}}",
    /// returns the native JSON type. If it contains text + expressions ("Hello {{name}}"),
    /// returns an interpolated string.
    /// </summary>
    public static JsonNode? Evaluate(string template, EvaluationContext ctx)
    {
        if (string.IsNullOrEmpty(template)) return JsonValue.Create(template);

        // Fast path: no expressions
        if (!template.Contains("{{"))
            return JsonValue.Create(template);

        // Check if it's a single pure expression (entire string is {{...}})
        var trimmed = template.AsSpan().Trim();
        if (trimmed.StartsWith("{{") && trimmed.EndsWith("}}") && !ContainsInner(trimmed))
        {
            var expr = trimmed[2..^2].ToString().Trim();
            return EvaluateExpression(expr, ctx);
        }

        // Mixed interpolation → string result
        var sb = new StringBuilder();
        int pos = 0;
        while (pos < template.Length)
        {
            int start = template.IndexOf("{{", pos, StringComparison.Ordinal);
            if (start < 0)
            {
                sb.Append(template, pos, template.Length - pos);
                break;
            }

            sb.Append(template, pos, start - pos);

            int end = template.IndexOf("}}", start + 2, StringComparison.Ordinal);
            if (end < 0)
            {
                sb.Append(template, start, template.Length - start);
                break;
            }

            var expr = template.Substring(start + 2, end - start - 2).Trim();
            var val = EvaluateExpression(expr, ctx);
            sb.Append(TemplateFilters.NodeToString(val));

            pos = end + 2;
        }

        return JsonValue.Create(sb.ToString());
    }

    /// <summary>
    /// Evaluates a single expression like "path.to.value | filter: arg | filter2".
    /// The returned node is always detached (deep-cloned from its source) so it
    /// can be safely inserted into the output tree.
    /// </summary>
    private static JsonNode? EvaluateExpression(string expr, EvaluationContext ctx)
    {
        var (path, filters) = ParseExpression(expr);
        var value = ctx.Resolve(path);

        foreach (var (filterName, filterArg) in filters)
        {
            value = TemplateFilters.Apply(filterName, value, filterArg);
        }

        // Deep-clone to detach from source (data model or scope) tree
        return value?.DeepClone();
    }

    /// <summary>
    /// Parses "path.to.value | filter: arg | filter2" into a path and filter chain.
    /// </summary>
    internal static (string path, List<(string name, string? arg)> filters) ParseExpression(string expr)
    {
        var parts = expr.Split('|');
        var path = parts[0].Trim();
        var filters = new List<(string name, string? arg)>();

        for (int i = 1; i < parts.Length; i++)
        {
            var filterExpr = parts[i].Trim();
            var colonIdx = filterExpr.IndexOf(':');
            if (colonIdx >= 0)
            {
                var name = filterExpr[..colonIdx].Trim();
                var arg = filterExpr[(colonIdx + 1)..].Trim();
                filters.Add((name, arg));
            }
            else
            {
                filters.Add((filterExpr, null));
            }
        }

        return (path, filters);
    }

    /// <summary>
    /// Checks if the span contains another {{ between the outer {{ and }}.
    /// Used to detect mixed interpolation vs. pure expression.
    /// </summary>
    private static bool ContainsInner(ReadOnlySpan<char> span)
    {
        // Skip the opening {{, search for another {{ before the closing }}
        var inner = span[2..^2];
        return inner.IndexOf("{{") >= 0;
    }
}
