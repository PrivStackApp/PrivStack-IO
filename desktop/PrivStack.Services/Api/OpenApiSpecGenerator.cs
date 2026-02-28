using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using PrivStack.Services.Plugin;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Services.Api;

/// <summary>
/// Generates an OpenAPI 3.1.0 specification from all registered <see cref="IApiProvider"/> routes
/// plus the shell's built-in routes (/api/v1/status, /api/v1/routes).
/// </summary>
public static class OpenApiSpecGenerator
{
    public static JsonObject Generate(IPluginRegistry pluginRegistry, int port)
    {
        var version = Assembly.GetEntryAssembly()?.GetName().Version;
        var versionStr = version != null ? $"{version.Major}.{version.Minor}.{version.Build}" : "1.0.0";

        var spec = new JsonObject
        {
            ["openapi"] = "3.1.0",
            ["info"] = new JsonObject
            {
                ["title"] = "PrivStack Local API",
                ["version"] = versionStr,
                ["description"] = "Local HTTP API for programmatic access to PrivStack data. Runs on localhost only.",
            },
            ["servers"] = new JsonArray
            {
                new JsonObject
                {
                    ["url"] = $"http://127.0.0.1:{port}",
                    ["description"] = "Local server",
                },
            },
        };

        var paths = new JsonObject();

        // Shell routes
        AddShellRoutes(paths);

        // Plugin routes
        var providers = pluginRegistry.GetCapabilityProviders<IApiProvider>();
        foreach (var provider in providers)
            AddProviderRoutes(paths, provider);

        spec["paths"] = paths;

        spec["components"] = new JsonObject
        {
            ["securitySchemes"] = new JsonObject
            {
                ["apiKey"] = new JsonObject
                {
                    ["type"] = "apiKey",
                    ["in"] = "header",
                    ["name"] = "X-API-Key",
                    ["description"] = "API key from PrivStack Developer Settings",
                },
            },
        };

        spec["security"] = new JsonArray
        {
            new JsonObject { ["apiKey"] = new JsonArray() },
        };

        return spec;
    }

    public static string GenerateJson(JsonObject spec)
    {
        return JsonSerializer.Serialize(spec, new JsonSerializerOptions { WriteIndented = true });
    }

    public static string GenerateYaml(JsonObject spec)
    {
        var sb = new StringBuilder();
        WriteYamlNode(sb, spec, 0, isRoot: true);
        return sb.ToString();
    }

    private static void AddShellRoutes(JsonObject paths)
    {
        // GET /api/v1/status — no auth required
        paths["/api/v1/status"] = new JsonObject
        {
            ["get"] = new JsonObject
            {
                ["summary"] = "Health check",
                ["operationId"] = "status",
                ["tags"] = new JsonArray { "shell" },
                ["security"] = new JsonArray(), // no auth
                ["responses"] = new JsonObject
                {
                    ["200"] = new JsonObject
                    {
                        ["description"] = "Server is running",
                        ["content"] = new JsonObject
                        {
                            ["application/json"] = new JsonObject
                            {
                                ["example"] = ParseJsonSafe("""{"status":"ok","version":"1"}"""),
                            },
                        },
                    },
                },
            },
        };

        // GET /api/v1/routes — auth required
        paths["/api/v1/routes"] = new JsonObject
        {
            ["get"] = new JsonObject
            {
                ["summary"] = "List all registered API routes",
                ["operationId"] = "routes",
                ["tags"] = new JsonArray { "shell" },
                ["responses"] = new JsonObject
                {
                    ["200"] = new JsonObject
                    {
                        ["description"] = "Route manifest",
                        ["content"] = new JsonObject
                        {
                            ["application/json"] = new JsonObject
                            {
                                ["example"] = ParseJsonSafe(
                                    """{"routes":[{"method":"GET","path":"/api/v1/tasks","description":"List all tasks","plugin":"tasks"}]}"""),
                            },
                        },
                    },
                },
            },
        };
    }

    private static void AddProviderRoutes(JsonObject paths, IApiProvider provider)
    {
        var slug = provider.ApiSlug;
        var routes = provider.GetRoutes();

        foreach (var route in routes)
        {
            var fullPath = string.IsNullOrEmpty(route.Path)
                ? $"/api/v1/{slug}"
                : $"/api/v1/{slug}/{route.Path}";

            var method = route.Method.ToString().ToLowerInvariant();

            // Ensure path entry exists (multiple methods can share a path)
            if (paths[fullPath] is not JsonObject pathItem)
            {
                pathItem = new JsonObject();
                paths[fullPath] = pathItem;
            }

            var operation = new JsonObject();

            if (!string.IsNullOrEmpty(route.Description))
                operation["summary"] = route.Description;

            operation["operationId"] = route.RouteId;
            operation["tags"] = new JsonArray { slug };

            // Parameters: path params + query params
            var parameters = new JsonArray();

            // Path parameters from {param} placeholders
            foreach (System.Text.RegularExpressions.Match match in
                System.Text.RegularExpressions.Regex.Matches(route.Path ?? "", @"\{(\w+)\}"))
            {
                parameters.Add(new JsonObject
                {
                    ["name"] = match.Groups[1].Value,
                    ["in"] = "path",
                    ["required"] = true,
                    ["schema"] = new JsonObject { ["type"] = "string" },
                });
            }

            // Query parameters from QueryParamDocs
            if (route.QueryParamDocs is { Count: > 0 })
            {
                foreach (var doc in route.QueryParamDocs)
                {
                    var colonIdx = doc.IndexOf(':');
                    var name = colonIdx > 0 ? doc[..colonIdx].Trim() : doc.Trim();
                    var desc = colonIdx > 0 ? doc[(colonIdx + 1)..].Trim() : null;

                    var param = new JsonObject
                    {
                        ["name"] = name,
                        ["in"] = "query",
                        ["required"] = false,
                        ["schema"] = new JsonObject { ["type"] = "string" },
                    };
                    if (desc != null)
                        param["description"] = desc;

                    parameters.Add(param);
                }
            }

            if (parameters.Count > 0)
                operation["parameters"] = parameters;

            // Request body from RequestExample
            if (!string.IsNullOrEmpty(route.RequestExample))
            {
                var exampleNode = ParseJsonSafe(route.RequestExample);
                operation["requestBody"] = new JsonObject
                {
                    ["required"] = true,
                    ["content"] = new JsonObject
                    {
                        ["application/json"] = new JsonObject
                        {
                            ["schema"] = new JsonObject { ["type"] = "object" },
                            ["example"] = exampleNode,
                        },
                    },
                };
            }

            // Response from ResponseExample
            var responseContent = new JsonObject
            {
                ["description"] = "Successful response",
            };

            if (!string.IsNullOrEmpty(route.ResponseExample))
            {
                responseContent["content"] = new JsonObject
                {
                    ["application/json"] = new JsonObject
                    {
                        ["schema"] = new JsonObject { ["type"] = "object" },
                        ["example"] = ParseJsonSafe(route.ResponseExample),
                    },
                };
            }

            operation["responses"] = new JsonObject
            {
                ["200"] = responseContent,
            };

            pathItem[method] = operation;
        }
    }

    private static JsonNode? ParseJsonSafe(string json)
    {
        try
        {
            return JsonNode.Parse(json);
        }
        catch
        {
            return JsonValue.Create(json);
        }
    }

    // ── Simple YAML emitter ──────────────────────────────────────

    private static void WriteYamlNode(StringBuilder sb, JsonNode? node, int indent, bool isRoot = false)
    {
        switch (node)
        {
            case JsonObject obj:
                WriteYamlObject(sb, obj, indent, isRoot);
                break;
            case JsonArray arr:
                WriteYamlArray(sb, arr, indent);
                break;
            case JsonValue val:
                WriteYamlValue(sb, val);
                break;
            default:
                sb.Append("null");
                break;
        }
    }

    private static void WriteYamlObject(StringBuilder sb, JsonObject obj, int indent, bool isRoot)
    {
        var prefix = new string(' ', indent);
        var first = true;

        foreach (var kvp in obj)
        {
            if (!isRoot || !first)
                sb.Append(prefix);
            first = false;

            var key = kvp.Key;
            // Quote keys that contain special YAML characters
            if (key.Contains('/') || key.Contains('{') || key.Contains('}'))
                key = $"\"{key}\"";

            sb.Append(key);
            sb.Append(':');

            if (kvp.Value is JsonObject childObj)
            {
                if (childObj.Count == 0)
                {
                    sb.AppendLine(" {}");
                }
                else
                {
                    sb.AppendLine();
                    WriteYamlObject(sb, childObj, indent + 2, false);
                }
            }
            else if (kvp.Value is JsonArray childArr)
            {
                if (childArr.Count == 0)
                {
                    sb.AppendLine(" []");
                }
                else
                {
                    sb.AppendLine();
                    WriteYamlArray(sb, childArr, indent + 2);
                }
            }
            else
            {
                sb.Append(' ');
                WriteYamlValue(sb, kvp.Value as JsonValue);
                sb.AppendLine();
            }
        }
    }

    private static void WriteYamlArray(StringBuilder sb, JsonArray arr, int indent)
    {
        var prefix = new string(' ', indent);

        foreach (var item in arr)
        {
            sb.Append(prefix);
            sb.Append("- ");

            if (item is JsonObject childObj)
            {
                if (childObj.Count == 0)
                {
                    sb.AppendLine("{}");
                }
                else
                {
                    // First key on same line as "- ", rest indented
                    var firstKey = true;
                    foreach (var kvp in childObj)
                    {
                        if (!firstKey)
                            sb.Append(prefix).Append("  ");
                        firstKey = false;

                        var key = kvp.Key;
                        if (key.Contains('/') || key.Contains('{') || key.Contains('}'))
                            key = $"\"{key}\"";

                        sb.Append(key);
                        sb.Append(':');

                        if (kvp.Value is JsonObject nestedObj)
                        {
                            if (nestedObj.Count == 0)
                            {
                                sb.AppendLine(" {}");
                            }
                            else
                            {
                                sb.AppendLine();
                                WriteYamlObject(sb, nestedObj, indent + 4, false);
                            }
                        }
                        else if (kvp.Value is JsonArray nestedArr)
                        {
                            if (nestedArr.Count == 0)
                            {
                                sb.AppendLine(" []");
                            }
                            else
                            {
                                sb.AppendLine();
                                WriteYamlArray(sb, nestedArr, indent + 4);
                            }
                        }
                        else
                        {
                            sb.Append(' ');
                            WriteYamlValue(sb, kvp.Value as JsonValue);
                            sb.AppendLine();
                        }
                    }
                }
            }
            else if (item is JsonArray childArr)
            {
                sb.AppendLine();
                WriteYamlArray(sb, childArr, indent + 2);
            }
            else
            {
                WriteYamlValue(sb, item as JsonValue);
                sb.AppendLine();
            }
        }
    }

    private static void WriteYamlValue(StringBuilder sb, JsonValue? value)
    {
        if (value == null)
        {
            sb.Append("null");
            return;
        }

        var element = value.GetValue<JsonElement>();
        switch (element.ValueKind)
        {
            case JsonValueKind.String:
                var str = element.GetString() ?? "";
                // Quote strings that could be misinterpreted by YAML parsers
                if (str.Length == 0 ||
                    str.Contains(':') || str.Contains('#') || str.Contains('\n') ||
                    str.Contains('"') || str.Contains('\'') ||
                    str.StartsWith('{') || str.StartsWith('[') || str.StartsWith('*') ||
                    str.StartsWith('&') || str.StartsWith('!') ||
                    str is "true" or "false" or "null" or "yes" or "no" or "on" or "off" ||
                    double.TryParse(str, out _))
                {
                    sb.Append('"');
                    sb.Append(str.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n"));
                    sb.Append('"');
                }
                else
                {
                    sb.Append(str);
                }
                break;
            case JsonValueKind.Number:
                sb.Append(element.GetRawText());
                break;
            case JsonValueKind.True:
                sb.Append("true");
                break;
            case JsonValueKind.False:
                sb.Append("false");
                break;
            default:
                sb.Append("null");
                break;
        }
    }
}
