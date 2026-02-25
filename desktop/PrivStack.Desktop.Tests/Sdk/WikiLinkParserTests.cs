namespace PrivStack.Desktop.Tests.Sdk;

using System.Text.Json;
using PrivStack.Sdk.Helpers;

public class WikiLinkParserTests
{
    // =========================================================================
    // ParseLinks — wiki-link format
    // =========================================================================

    [Fact]
    public void ParseLinks_extracts_wiki_link()
    {
        var content = "See [[page:abc-123|My Note]] for details.";
        var links = WikiLinkParser.ParseLinks(content);
        links.Should().HaveCount(1);
        links[0].LinkType.Should().Be("page");
        links[0].EntityId.Should().Be("abc-123");
    }

    [Fact]
    public void ParseLinks_extracts_multiple_wiki_links()
    {
        var content = "Link to [[task:t1|Task 1]] and [[contact:c2|John Doe]].";
        var links = WikiLinkParser.ParseLinks(content);
        links.Should().HaveCount(2);
        links[0].LinkType.Should().Be("task");
        links[1].LinkType.Should().Be("contact");
    }

    [Fact]
    public void ParseLinks_extracts_hyphenated_link_types()
    {
        var content = "See [[rss-article:ra-1|RSS Article]] and [[web-clip:wc-1|Web Clip]].";
        var links = WikiLinkParser.ParseLinks(content);
        links.Should().HaveCount(2);
        links[0].LinkType.Should().Be("rss-article");
        links[1].LinkType.Should().Be("web-clip");
    }

    [Fact]
    public void ParseLinks_returns_empty_for_no_links()
    {
        WikiLinkParser.ParseLinks("plain text with no links").Should().BeEmpty();
    }

    [Fact]
    public void ParseLinks_returns_empty_for_empty_string()
    {
        WikiLinkParser.ParseLinks("").Should().BeEmpty();
    }

    // =========================================================================
    // ParseLinks — privstack:// URL format
    // =========================================================================

    [Fact]
    public void ParseLinks_extracts_privstack_url()
    {
        var content = "Check [My Task](privstack://task/abcdef-1234-5678).";
        var links = WikiLinkParser.ParseLinks(content);
        links.Should().HaveCount(1);
        links[0].LinkType.Should().Be("task");
        links[0].EntityId.Should().Be("abcdef-1234-5678");
    }

    [Fact]
    public void ParseLinks_extracts_mixed_link_formats()
    {
        var content = "[[page:p1|Page]] and [Contact](privstack://contact/c1-2345-6789).";
        var links = WikiLinkParser.ParseLinks(content);
        links.Should().HaveCount(2);
        links[0].LinkType.Should().Be("page");
        links[1].LinkType.Should().Be("contact");
    }

    // =========================================================================
    // ParsedLink
    // =========================================================================

    [Fact]
    public void ParsedLink_composite_key()
    {
        var link = new ParsedLink("task", "t-123", 0, 10);
        link.CompositeKey.Should().Be("task:t-123");
    }

    // =========================================================================
    // ExtractContentFromEntity
    // =========================================================================

    [Fact]
    public void ExtractContentFromEntity_extracts_string_content()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""{"content": "Hello world"}""");
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().Be("Hello world");
    }

    [Fact]
    public void ExtractContentFromEntity_extracts_description()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""{"description": "A task"}""");
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().Be("A task");
    }

    [Fact]
    public void ExtractContentFromEntity_returns_null_when_no_content_fields()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""{"id": "123", "title": "Test"}""");
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().BeNull();
    }

    [Fact]
    public void ExtractContentFromEntity_extracts_from_nested_json()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""
        {
            "content": {
                "blocks": [
                    {"text": "paragraph one"},
                    {"text": "paragraph two"}
                ]
            }
        }
        """);
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().Contain("paragraph one");
        result.Should().Contain("paragraph two");
    }

    [Fact]
    public void ExtractContentFromEntity_uses_custom_fields()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""{"summary": "Custom field text"}""");
        var result = WikiLinkParser.ExtractContentFromEntity(json, ["summary"]);
        result.Should().Be("Custom field text");
    }

    [Fact]
    public void ExtractContentFromEntity_joins_multiple_fields()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""
        {"content": "First", "description": "Second"}
        """);
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().Contain("First");
        result.Should().Contain("Second");
    }

    [Fact]
    public void ExtractContentFromEntity_skips_empty_strings()
    {
        var json = JsonSerializer.Deserialize<JsonElement>("""
        {"content": "", "description": "Non-empty"}
        """);
        var result = WikiLinkParser.ExtractContentFromEntity(json);
        result.Should().Be("Non-empty");
    }

    // =========================================================================
    // ExtractSnippet
    // =========================================================================

    [Fact]
    public void ExtractSnippet_returns_context_around_match()
    {
        var content = "The quick brown fox jumps over the lazy dog near the big red barn.";
        var snippet = WikiLinkParser.ExtractSnippet(content, 30, 20);
        snippet.Should().NotBeNullOrWhiteSpace();
    }

    [Fact]
    public void ExtractSnippet_adds_ellipsis_for_truncation()
    {
        var content = "AAAA " + new string('B', 200) + " CCCC";
        var snippet = WikiLinkParser.ExtractSnippet(content, 100, 30);
        snippet.Should().StartWith("...");
        snippet.Should().EndWith("...");
    }

    [Fact]
    public void ExtractSnippet_no_ellipsis_at_start_when_match_is_at_beginning()
    {
        var content = "Hello world, this is a test with more text after it.";
        var snippet = WikiLinkParser.ExtractSnippet(content, 0, 20);
        snippet.Should().NotStartWith("...");
    }

    [Fact]
    public void ExtractSnippet_strips_wiki_link_markup()
    {
        var content = "Referencing [[page:p1|My Page]] in this context.";
        var snippet = WikiLinkParser.ExtractSnippet(content, 12, 40);
        snippet.Should().Contain("My Page");
        snippet.Should().NotContain("[[");
    }

    // =========================================================================
    // Regex patterns
    // =========================================================================

    [Fact]
    public void WikiLinkPattern_matches_standard_format()
    {
        var match = WikiLinkParser.WikiLinkPattern.Match("[[page:abc-123|Title]]");
        match.Success.Should().BeTrue();
        match.Groups[1].Value.Should().Be("page");
        match.Groups[2].Value.Should().Be("abc-123");
    }

    [Fact]
    public void PrivstackUrlPattern_matches_standard_format()
    {
        var match = WikiLinkParser.PrivstackUrlPattern.Match("privstack://task/abcdef-1234-5678");
        match.Success.Should().BeTrue();
        match.Groups[1].Value.Should().Be("task");
        match.Groups[2].Value.Should().Be("abcdef-1234-5678");
    }

    [Fact]
    public void WikiLinkPattern_rejects_uppercase_types()
    {
        var match = WikiLinkParser.WikiLinkPattern.Match("[[Page:abc|Title]]");
        match.Success.Should().BeFalse();
    }
}
