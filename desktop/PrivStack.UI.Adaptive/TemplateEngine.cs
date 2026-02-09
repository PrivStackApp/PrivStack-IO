// ============================================================================
// File: TemplateEngine.cs
// Description: JSON template engine that evaluates a declarative template
//              against a data model, producing a component tree JSON for
//              AdaptiveViewRenderer. Supports $for loops, $if conditionals,
//              and {{expression | filter}} interpolation.
// ============================================================================

using System.Text.Json;
using System.Text.Json.Nodes;
using Serilog;

namespace PrivStack.UI.Adaptive;

/// <summary>
/// Evaluates a JSON template against a data model, producing a component tree.
/// Thread-safe: the parsed template is immutable; evaluation creates new nodes.
/// </summary>
public sealed class TemplateEngine
{
    private static readonly ILogger _log = Log.ForContext<TemplateEngine>();

    private readonly JsonNode _template;

    /// <summary>
    /// Creates a template engine from a JSON template string.
    /// </summary>
    /// <exception cref="JsonException">If the template is not valid JSON.</exception>
    public TemplateEngine(string templateJson)
    {
        _template = JsonNode.Parse(templateJson)
            ?? throw new JsonException("Template parsed to null");
    }

    /// <summary>
    /// Evaluates the template against a data model JSON string.
    /// Returns the component tree JSON string ready for AdaptiveViewRenderer.
    /// </summary>
    public string Evaluate(string dataModelJson)
    {
        var data = JsonNode.Parse(dataModelJson) as JsonObject ?? new JsonObject();
        var ctx = new EvaluationContext(data);
        var result = EvaluateNode(_template, ctx);
        return result?.ToJsonString() ?? "{}";
    }

    /// <summary>
    /// Evaluates the template against a pre-parsed data model.
    /// </summary>
    internal JsonNode? Evaluate(JsonObject data)
    {
        var ctx = new EvaluationContext(data);
        return EvaluateNode(_template, ctx);
    }

    private JsonNode? EvaluateNode(JsonNode? node, EvaluationContext ctx)
    {
        return node switch
        {
            JsonObject obj when obj.ContainsKey("$for") => EvaluateForLoop(obj, ctx),
            JsonObject obj when obj.ContainsKey("$if") => EvaluateConditional(obj, ctx),
            JsonObject obj => EvaluateObject(obj, ctx),
            JsonArray arr => EvaluateArray(arr, ctx),
            JsonValue val when val.TryGetValue(out string? s) && s is not null && s.Contains("{{") =>
                ExpressionEvaluator.Evaluate(s, ctx),
            _ => node?.DeepClone(),
        };
    }

    private JsonNode? EvaluateObject(JsonObject obj, EvaluationContext ctx)
    {
        var result = new JsonObject();
        foreach (var (key, value) in obj)
        {
            result[key] = Detach(EvaluateNode(value, ctx));
        }
        return result;
    }

    private JsonNode? EvaluateArray(JsonArray arr, EvaluationContext ctx)
    {
        var result = new JsonArray();
        foreach (var element in arr)
        {
            if (element is JsonObject obj && obj.ContainsKey("$for"))
            {
                // $for in an array context: splice the loop results into the parent array
                var loopResult = EvaluateForLoop(obj, ctx);
                if (loopResult is JsonArray loopArr)
                {
                    foreach (var item in loopArr)
                    {
                        result.Add(item?.DeepClone());
                    }
                }
                else if (loopResult is not null)
                {
                    result.Add(Detach(loopResult));
                }
            }
            else if (element is JsonObject ifObj && ifObj.ContainsKey("$if"))
            {
                // $if in array context: add the result if non-null
                var ifResult = EvaluateConditional(ifObj, ctx);
                if (ifResult is not null)
                {
                    result.Add(Detach(ifResult));
                }
            }
            else
            {
                result.Add(Detach(EvaluateNode(element, ctx)));
            }
        }
        return result;
    }

    /// <summary>
    /// Ensures a node can be inserted into a new parent by deep-cloning if it
    /// already belongs to another tree. System.Text.Json.Nodes enforces
    /// single-parent ownership.
    /// </summary>
    private static JsonNode? Detach(JsonNode? node)
    {
        if (node is null) return null;
        return node.Parent is not null ? node.DeepClone() : node;
    }

    private JsonNode? EvaluateForLoop(JsonObject node, EvaluationContext ctx)
    {
        var itemVar = GetStringProp(node, "$for");
        var collectionPath = GetStringProp(node, "$in");
        var template = node["$template"];
        var emptyTemplate = node["$empty"];

        if (itemVar is null || collectionPath is null || template is null)
        {
            _log.Warning("$for loop missing required properties ($for, $in, $template)");
            return null;
        }

        var collectionNode = ExpressionEvaluator.Evaluate($"{{{{{collectionPath}}}}}", ctx);
        if (collectionNode is not JsonArray collection || collection.Count == 0)
        {
            return emptyTemplate is not null ? EvaluateNode(emptyTemplate, ctx) : null;
        }

        var result = new JsonArray();
        for (int i = 0; i < collection.Count; i++)
        {
            var item = collection[i];
            var scope = new Dictionary<string, JsonNode?>
            {
                [itemVar] = item?.DeepClone(),
                ["loop"] = new JsonObject
                {
                    ["index"] = i,
                    ["first"] = i == 0,
                    ["last"] = i == collection.Count - 1,
                    ["length"] = collection.Count,
                },
            };

            ctx.PushScope(scope);
            var evaluated = EvaluateNode(template, ctx);
            ctx.PopScope();

            if (evaluated is not null)
            {
                result.Add(evaluated);
            }
        }

        return result;
    }

    private JsonNode? EvaluateConditional(JsonObject node, EvaluationContext ctx)
    {
        var conditionPath = GetStringProp(node, "$if");
        if (conditionPath is null) return null;

        var conditionValue = ExpressionEvaluator.Evaluate($"{{{{{conditionPath}}}}}", ctx);
        var isTruthy = TemplateFilters.IsTruthy(conditionValue);

        if (isTruthy)
        {
            var thenNode = node["$then"];
            return thenNode is not null ? EvaluateNode(thenNode, ctx) : null;
        }
        else
        {
            var elseNode = node["$else"];
            return elseNode is not null ? EvaluateNode(elseNode, ctx) : null;
        }
    }

    private static string? GetStringProp(JsonObject obj, string key)
    {
        if (obj.TryGetPropertyValue(key, out var node) && node is JsonValue val && val.TryGetValue(out string? s))
            return s;
        return null;
    }
}
