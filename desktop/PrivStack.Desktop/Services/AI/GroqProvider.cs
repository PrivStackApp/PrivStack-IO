using PrivStack.Sdk;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Groq cloud inference provider. Fast inference with no-training policy — StandardApi tier.
/// API key stored encrypted in vault ("ai-vault", "groq-api-key").
/// </summary>
internal sealed class GroqProvider : OpenAiCompatibleProviderBase
{
    private static readonly AiModelInfo[] Models =
    [
        new() { Id = "llama-3.3-70b-versatile", DisplayName = "Llama 3.3 70B", ContextWindowTokens = 128_000 },
        new() { Id = "llama-3.1-8b-instant", DisplayName = "Llama 3.1 8B", ContextWindowTokens = 128_000 },
        new() { Id = "mixtral-8x7b-32768", DisplayName = "Mixtral 8x7B", ContextWindowTokens = 32_768 },
    ];

    public GroqProvider(IPrivStackSdk sdk) : base(sdk) { }

    public override string Id => "groq";
    public override string DisplayName => "Groq";
    public override PrivacyTier PrivacyTier => PrivacyTier.StandardApi;
    public override IReadOnlyList<AiModelInfo> AvailableModels => Models;

    protected override string CompletionUrl => "https://api.groq.com/openai/v1/chat/completions";
    protected override string VaultBlobId => "groq-api-key";
    protected override string DefaultModelId => "llama-3.3-70b-versatile";
}
