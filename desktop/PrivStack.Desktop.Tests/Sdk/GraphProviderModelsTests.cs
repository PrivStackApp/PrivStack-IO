namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;

public class GraphProviderModelsTests
{
    // =========================================================================
    // GraphNodeContribution
    // =========================================================================

    [Fact]
    public void GraphNodeContribution_composite_key()
    {
        var node = new GraphNodeContribution
        {
            Id = "abc-123",
            Title = "Test Note",
            LinkType = "page",
            NodeType = "note"
        };

        node.CompositeKey.Should().Be("page:abc-123");
    }

    [Fact]
    public void GraphNodeContribution_defaults()
    {
        var node = new GraphNodeContribution
        {
            Id = "1",
            Title = "Test",
            LinkType = "task",
            NodeType = "task"
        };

        node.Icon.Should().BeNull();
        node.Tags.Should().BeEmpty();
        node.ModifiedAt.Should().Be(default);
    }

    [Fact]
    public void GraphNodeContribution_with_all_fields()
    {
        var now = DateTimeOffset.UtcNow;
        var node = new GraphNodeContribution
        {
            Id = "t-1",
            Title = "Build feature",
            LinkType = "task",
            NodeType = "task",
            Icon = "CheckSquare",
            Tags = new List<string> { "work", "feature" },
            ModifiedAt = now
        };

        node.Title.Should().Be("Build feature");
        node.Icon.Should().Be("CheckSquare");
        node.Tags.Should().HaveCount(2);
        node.ModifiedAt.Should().Be(now);
    }

    // =========================================================================
    // GraphEdgeContribution
    // =========================================================================

    [Fact]
    public void GraphEdgeContribution_construction()
    {
        var edge = new GraphEdgeContribution
        {
            SourceKey = "page:p1",
            TargetKey = "contact:c1",
            EdgeType = "mentions"
        };

        edge.SourceKey.Should().Be("page:p1");
        edge.TargetKey.Should().Be("contact:c1");
        edge.EdgeType.Should().Be("mentions");
        edge.Label.Should().BeNull();
    }

    [Fact]
    public void GraphEdgeContribution_with_label()
    {
        var edge = new GraphEdgeContribution
        {
            SourceKey = "contact:c1",
            TargetKey = "contact:c2",
            EdgeType = "company",
            Label = "Works at"
        };

        edge.Label.Should().Be("Works at");
    }

    // =========================================================================
    // ContentField
    // =========================================================================

    [Fact]
    public void ContentField_construction()
    {
        var field = new ContentField
        {
            OwnerKey = "page:p1",
            Content = "This is page content with [[task:t1|My Task]]."
        };

        field.OwnerKey.Should().Be("page:p1");
        field.Content.Should().Contain("[[task:t1|My Task]]");
    }

    // =========================================================================
    // ExplicitLinkContribution
    // =========================================================================

    [Fact]
    public void ExplicitLinkContribution_construction()
    {
        var link = new ExplicitLinkContribution
        {
            SourceKey = "task:t1",
            TargetKey = "task:t2"
        };

        link.SourceKey.Should().Be("task:t1");
        link.TargetKey.Should().Be("task:t2");
    }

    // =========================================================================
    // GraphContribution
    // =========================================================================

    [Fact]
    public void GraphContribution_defaults_to_empty()
    {
        var contribution = new GraphContribution();
        contribution.Nodes.Should().BeEmpty();
        contribution.StructuralEdges.Should().BeEmpty();
        contribution.ContentFields.Should().BeEmpty();
        contribution.ExplicitLinks.Should().BeEmpty();
    }

    [Fact]
    public void GraphContribution_with_data()
    {
        var contribution = new GraphContribution
        {
            Nodes = new List<GraphNodeContribution>
            {
                new() { Id = "1", Title = "Note", LinkType = "page", NodeType = "note" },
                new() { Id = "2", Title = "Task", LinkType = "task", NodeType = "task" }
            },
            StructuralEdges = new List<GraphEdgeContribution>
            {
                new() { SourceKey = "page:1", TargetKey = "task:2", EdgeType = "reference" }
            },
            ContentFields = new List<ContentField>
            {
                new() { OwnerKey = "page:1", Content = "References [[task:2|Task]]" }
            },
            ExplicitLinks = new List<ExplicitLinkContribution>
            {
                new() { SourceKey = "page:1", TargetKey = "task:2" }
            }
        };

        contribution.Nodes.Should().HaveCount(2);
        contribution.StructuralEdges.Should().HaveCount(1);
        contribution.ContentFields.Should().HaveCount(1);
        contribution.ExplicitLinks.Should().HaveCount(1);
    }
}
