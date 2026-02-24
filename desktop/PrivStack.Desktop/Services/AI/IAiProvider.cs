using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Internal provider interface for AI backends.
/// Each provider handles a specific API (OpenAI, Anthropic, Gemini, local LLM).
/// </summary>
internal interface IAiProvider
{
    string Id { get; }
    string DisplayName { get; }
    bool IsConfigured { get; }
    bool IsLocal { get; }
    PrivacyTier PrivacyTier { get; }
    IReadOnlyList<AiModelInfo> AvailableModels { get; }

    Task<AiResponse> CompleteAsync(AiRequest request, string? modelOverride, CancellationToken ct);

    /// <summary>
    /// Streaming variant — calls <paramref name="onToken"/> as each token is generated.
    /// Default implementation falls back to non-streaming <see cref="CompleteAsync"/>.
    /// </summary>
    Task<AiResponse> StreamCompleteAsync(AiRequest request, string? modelOverride,
        Action<string> onToken, CancellationToken ct)
        => CompleteAsync(request, modelOverride, ct);

    Task<bool> ValidateAsync(CancellationToken ct = default);
}
