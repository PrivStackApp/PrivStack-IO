using PrivStack.Sdk;
using PrivStack.Sdk.Services;

namespace PrivStack.Desktop.Services.AI;

/// <summary>
/// Mistral AI chat completion provider. EU-based, GDPR-native — HighPrivacy tier.
/// API key stored encrypted in vault ("ai-vault", "mistral-api-key").
/// </summary>
internal sealed class MistralProvider : OpenAiCompatibleProviderBase
{
    private static readonly AiModelInfo[] Models =
    [
        new() { Id = "mistral-large-latest", DisplayName = "Mistral Large", ContextWindowTokens = 128_000 },
        new() { Id = "mistral-medium-latest", DisplayName = "Mistral Medium", ContextWindowTokens = 32_000 },
        new() { Id = "mistral-small-latest", DisplayName = "Mistral Small", ContextWindowTokens = 32_000 },
    ];

    public MistralProvider(IPrivStackSdk sdk) : base(sdk) { }

    public override string Id => "mistral";
    public override string DisplayName => "Mistral AI";
    public override PrivacyTier PrivacyTier => PrivacyTier.HighPrivacy;
    public override IReadOnlyList<AiModelInfo> AvailableModels => Models;

    protected override string CompletionUrl => "https://api.mistral.ai/v1/chat/completions";
    protected override string VaultBlobId => "mistral-api-key";
    protected override string DefaultModelId => "mistral-small-latest";
}
