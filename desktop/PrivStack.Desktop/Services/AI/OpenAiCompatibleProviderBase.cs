using System.Text.Json;
using PrivStack.Sdk;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Base class for providers using the OpenAI-compatible chat completions API format.
/// Handles message building, completion execution, and vault-based API key management.
/// </summary>
internal abstract class OpenAiCompatibleProviderBase : AiProviderBase
{
    private readonly IPrivStackSdk _sdk;
    private string? _cachedApiKey;

    protected OpenAiCompatibleProviderBase(IPrivStackSdk sdk) => _sdk = sdk;

    protected abstract string CompletionUrl { get; }
    protected abstract string VaultBlobId { get; }
    protected abstract string DefaultModelId { get; }
    protected virtual string VaultId => "ai-vault";

    public override bool IsConfigured => GetApiKeySync() != null;

    public override async Task<bool> ValidateAsync(CancellationToken ct)
    {
        var key = await GetApiKeyAsync(ct);
        return !string.IsNullOrEmpty(key);
    }

    protected override async Task<AiResponse> ExecuteCompletionAsync(
        AiRequest request, string? modelOverride, CancellationToken ct)
    {
        var apiKey = await GetApiKeyAsync(ct)
            ?? throw new InvalidOperationException($"{DisplayName} API key not configured");

        var model = modelOverride ?? DefaultModelId;
        var messages = BuildMessages(request);
        var payload = new
        {
            model,
            messages,
            max_tokens = request.MaxTokens,
            temperature = request.Temperature
        };

        var headers = new Dictionary<string, string> { ["Authorization"] = $"Bearer {apiKey}" };
        using var doc = await PostJsonAsync(CompletionUrl, payload, headers, ct);
        var root = doc.RootElement;

        var content = root.GetProperty("choices")[0]
            .GetProperty("message")
            .GetProperty("content")
            .GetString();

        var tokensUsed = root.TryGetProperty("usage", out var usage)
            ? usage.GetProperty("total_tokens").GetInt32() : 0;

        return new AiResponse
        {
            Success = true,
            Content = content,
            ProviderUsed = Id,
            ModelUsed = model,
            TokensUsed = tokensUsed
        };
    }

    public void ClearCachedKey() => _cachedApiKey = null;

    private string? GetApiKeySync()
    {
        if (_cachedApiKey != null) return _cachedApiKey;
        try
        {
            var task = _sdk.VaultBlobRead(VaultId, VaultBlobId);
            if (task.Wait(TimeSpan.FromSeconds(2)))
            {
                _cachedApiKey = System.Text.Encoding.UTF8.GetString(task.Result);
                return _cachedApiKey;
            }
        }
        catch { /* vault locked or not initialized */ }
        return null;
    }

    protected async Task<string?> GetApiKeyAsync(CancellationToken ct)
    {
        if (_cachedApiKey != null) return _cachedApiKey;
        try
        {
            var bytes = await _sdk.VaultBlobRead(VaultId, VaultBlobId, ct);
            _cachedApiKey = System.Text.Encoding.UTF8.GetString(bytes);
            return _cachedApiKey;
        }
        catch { return null; }
    }

    private static List<object> BuildMessages(AiRequest request)
    {
        var messages = new List<object>
        {
            new { role = "system", content = request.SystemPrompt }
        };

        if (request.ConversationHistory is { Count: > 0 })
        {
            foreach (var msg in request.ConversationHistory)
                messages.Add(new { role = msg.Role, content = msg.Content });
        }

        messages.Add(new { role = "user", content = request.UserPrompt });
        return messages;
    }
}
