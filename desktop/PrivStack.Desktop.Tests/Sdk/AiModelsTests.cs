namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Services;

public class AiModelsTests
{
    // =========================================================================
    // PrivacyTier
    // =========================================================================

    [Theory]
    [InlineData(PrivacyTier.HighPrivacy)]
    [InlineData(PrivacyTier.StandardApi)]
    public void PrivacyTier_all_values(PrivacyTier tier)
    {
        Enum.IsDefined(tier).Should().BeTrue();
    }

    // =========================================================================
    // AiChatMessage
    // =========================================================================

    [Fact]
    public void AiChatMessage_construction()
    {
        var msg = new AiChatMessage { Role = "user", Content = "Hello" };
        msg.Role.Should().Be("user");
        msg.Content.Should().Be("Hello");
    }

    // =========================================================================
    // AiRequest
    // =========================================================================

    [Fact]
    public void AiRequest_defaults()
    {
        var req = new AiRequest
        {
            SystemPrompt = "You are helpful",
            UserPrompt = "Summarize this"
        };
        req.MaxTokens.Should().Be(1024);
        req.Temperature.Should().Be(0.7);
        req.FeatureId.Should().BeNull();
        req.ConversationHistory.Should().BeNull();
    }

    [Fact]
    public void AiRequest_with_all_fields()
    {
        var req = new AiRequest
        {
            SystemPrompt = "System",
            UserPrompt = "User",
            MaxTokens = 2048,
            Temperature = 0.3,
            FeatureId = "notes.summarize",
            ConversationHistory = new List<AiChatMessage>
            {
                new() { Role = "user", Content = "Hi" },
                new() { Role = "assistant", Content = "Hello" }
            }
        };
        req.MaxTokens.Should().Be(2048);
        req.Temperature.Should().Be(0.3);
        req.ConversationHistory.Should().HaveCount(2);
    }

    // =========================================================================
    // AiResponse
    // =========================================================================

    [Fact]
    public void AiResponse_success()
    {
        var resp = new AiResponse
        {
            Success = true,
            Content = "Here is a summary",
            ProviderUsed = "anthropic",
            ModelUsed = "claude-3",
            TokensUsed = 150,
            Duration = TimeSpan.FromMilliseconds(500)
        };
        resp.Success.Should().BeTrue();
        resp.Content.Should().Contain("summary");
        resp.TokensUsed.Should().Be(150);
    }

    [Fact]
    public void AiResponse_Failure_factory()
    {
        var resp = AiResponse.Failure("API key invalid");
        resp.Success.Should().BeFalse();
        resp.ErrorMessage.Should().Be("API key invalid");
        resp.Content.Should().BeNull();
    }

    [Fact]
    public void AiResponse_defaults()
    {
        var resp = new AiResponse { Success = true };
        resp.Content.Should().BeNull();
        resp.ErrorMessage.Should().BeNull();
        resp.ProviderUsed.Should().BeNull();
        resp.ModelUsed.Should().BeNull();
        resp.TokensUsed.Should().Be(0);
        resp.Duration.Should().Be(TimeSpan.Zero);
    }

    // =========================================================================
    // AiProviderInfo
    // =========================================================================

    [Fact]
    public void AiProviderInfo_defaults()
    {
        var info = new AiProviderInfo
        {
            Id = "ollama",
            DisplayName = "Ollama"
        };
        info.IsConfigured.Should().BeFalse();
        info.IsLocal.Should().BeFalse();
        info.PrivacyTier.Should().BeNull();
        info.AvailableModels.Should().BeEmpty();
    }

    [Fact]
    public void AiProviderInfo_with_models()
    {
        var info = new AiProviderInfo
        {
            Id = "ollama",
            DisplayName = "Ollama",
            IsConfigured = true,
            IsLocal = true,
            PrivacyTier = PrivacyTier.HighPrivacy,
            AvailableModels = new List<AiModelInfo>
            {
                new()
                {
                    Id = "llama3",
                    DisplayName = "Llama 3",
                    SizeBytes = 4_000_000_000,
                    IsDownloaded = true,
                    ContextWindowTokens = 8192
                }
            }
        };
        info.AvailableModels.Should().HaveCount(1);
        info.AvailableModels[0].SizeBytes.Should().Be(4_000_000_000);
    }

    // =========================================================================
    // AiModelInfo
    // =========================================================================

    [Fact]
    public void AiModelInfo_defaults()
    {
        var model = new AiModelInfo
        {
            Id = "gpt-4",
            DisplayName = "GPT-4"
        };
        model.SizeBytes.Should().Be(0);
        model.IsDownloaded.Should().BeFalse();
        model.ContextWindowTokens.Should().Be(0);
    }
}
