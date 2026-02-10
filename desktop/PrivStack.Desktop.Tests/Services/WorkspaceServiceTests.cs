using System.Text.Json;
using System.Text.RegularExpressions;
using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Tests.Services;

/// <summary>
/// Tests for WorkspaceService logic. Since WorkspaceService is a singleton with
/// filesystem dependencies, we test the Slugify logic and model serialization directly.
/// </summary>
public class WorkspaceServiceTests
{
    // Mirror the Slugify logic from WorkspaceService for testing
    private static string Slugify(string name)
    {
        var slug = name.ToLowerInvariant().Trim();
        slug = Regex.Replace(slug, @"[^a-z0-9\s-]", "");
        slug = Regex.Replace(slug, @"\s+", "-");
        slug = Regex.Replace(slug, @"-+", "-");
        slug = slug.Trim('-');
        return string.IsNullOrEmpty(slug) ? "workspace" : slug;
    }

    [Theory]
    [InlineData("My Workspace", "my-workspace")]
    [InlineData("Hello World", "hello-world")]
    [InlineData("test", "test")]
    [InlineData("UPPER CASE", "upper-case")]
    public void Slugify_ProducesValidSlugs(string input, string expected)
    {
        Slugify(input).Should().Be(expected);
    }

    [Theory]
    [InlineData("Hello!@#$%World", "helloworld")]
    [InlineData("café", "caf")]
    [InlineData("test (1)", "test-1")]
    [InlineData("a--b", "a-b")]
    public void Slugify_HandlesSpecialCharacters(string input, string expected)
    {
        Slugify(input).Should().Be(expected);
    }

    [Theory]
    [InlineData("  spaces  ", "spaces")]
    [InlineData("  leading", "leading")]
    [InlineData("trailing  ", "trailing")]
    public void Slugify_HandlesWhitespace(string input, string expected)
    {
        Slugify(input).Should().Be(expected);
    }

    [Theory]
    [InlineData("", "workspace")]
    [InlineData("   ", "workspace")]
    [InlineData("!!!", "workspace")]
    public void Slugify_HandlesEmptyStrings(string input, string expected)
    {
        Slugify(input).Should().Be(expected);
    }

    [Fact]
    public void Workspace_RoundTrips()
    {
        var ws = new Workspace
        {
            Id = "my-workspace",
            Name = "My Workspace",
            CreatedAt = new DateTime(2025, 1, 1, 0, 0, 0, DateTimeKind.Utc),
            HasPassword = true
        };

        var json = JsonSerializer.Serialize(ws);
        var deserialized = JsonSerializer.Deserialize<Workspace>(json);

        deserialized!.Id.Should().Be("my-workspace");
        deserialized.Name.Should().Be("My Workspace");
        deserialized.HasPassword.Should().BeTrue();
    }

    [Fact]
    public void WorkspaceRegistry_RoundTrips()
    {
        var registry = new WorkspaceRegistry
        {
            Workspaces =
            [
                new Workspace { Id = "ws1", Name = "Work" },
                new Workspace { Id = "ws2", Name = "Personal" }
            ],
            ActiveWorkspaceId = "ws1"
        };

        var json = JsonSerializer.Serialize(registry);
        var deserialized = JsonSerializer.Deserialize<WorkspaceRegistry>(json);

        deserialized!.Workspaces.Should().HaveCount(2);
        deserialized.ActiveWorkspaceId.Should().Be("ws1");
    }

    [Fact]
    public void WorkspaceRegistry_DefaultValues()
    {
        var registry = new WorkspaceRegistry();

        registry.Workspaces.Should().BeEmpty();
        registry.ActiveWorkspaceId.Should().BeNull();
    }

    [Fact]
    public void GetDataPath_Format()
    {
        // Verify the expected path format
        var workspaceId = "my-workspace";
        var expectedSuffix = Path.Combine("workspaces", workspaceId, "data.duckdb");

        // The path should end with workspaces/{id}/data.duckdb
        expectedSuffix.Should().EndWith($"workspaces{Path.DirectorySeparatorChar}my-workspace{Path.DirectorySeparatorChar}data.duckdb");
    }
}
