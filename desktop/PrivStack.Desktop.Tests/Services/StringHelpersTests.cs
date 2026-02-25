namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class StringHelpersTests
{
    [Fact]
    public void StripEmojis_removes_surrogate_pairs()
    {
        var input = "Hello 🌍 World";
        var result = StringHelpers.StripEmojis(input);
        result.Should().NotContain("🌍");
        result.Should().Contain("Hello");
        result.Should().Contain("World");
    }

    [Fact]
    public void StripEmojis_preserves_plain_text()
    {
        var input = "Hello World 123";
        StringHelpers.StripEmojis(input).Should().Be("Hello World 123");
    }

    [Fact]
    public void StripEmojis_returns_null_for_null()
    {
        StringHelpers.StripEmojis(null!).Should().BeNull();
    }

    [Fact]
    public void StripEmojis_returns_empty_for_empty()
    {
        StringHelpers.StripEmojis("").Should().BeEmpty();
    }

    [Fact]
    public void StripEmojis_preserves_accented_characters()
    {
        var input = "café résumé naïve";
        StringHelpers.StripEmojis(input).Should().Be("café résumé naïve");
    }

    [Fact]
    public void StripEmojis_preserves_cjk_characters()
    {
        var input = "你好世界 Hello";
        StringHelpers.StripEmojis(input).Should().Be("你好世界 Hello");
    }

    [Fact]
    public void StripEmojis_handles_emoji_only_string()
    {
        var input = "🎉🎊🎈";
        var result = StringHelpers.StripEmojis(input);
        result.Should().NotContain("🎉");
    }

    [Fact]
    public void StripEmojis_preserves_special_characters()
    {
        var input = "test@email.com #hashtag $100";
        StringHelpers.StripEmojis(input).Should().Be("test@email.com #hashtag $100");
    }
}
