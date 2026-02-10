using System.Text.Json;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Models;

public class WorkspaceModelTests
{
    [Fact]
    public void Workspace_JsonRoundTrip()
    {
        var ws = new Workspace
        {
            Id = "test-ws",
            Name = "Test Workspace",
            CreatedAt = new DateTime(2025, 6, 1, 12, 0, 0, DateTimeKind.Utc),
            HasPassword = true
        };

        var json = JsonSerializer.Serialize(ws);
        var deserialized = JsonSerializer.Deserialize<Workspace>(json);

        deserialized.Should().NotBeNull();
        deserialized!.Id.Should().Be("test-ws");
        deserialized.Name.Should().Be("Test Workspace");
        deserialized.HasPassword.Should().BeTrue();
    }

    [Fact]
    public void Workspace_DefaultValues()
    {
        var ws = new Workspace();

        ws.Id.Should().BeEmpty();
        ws.Name.Should().BeEmpty();
        ws.HasPassword.Should().BeFalse();
    }

    [Fact]
    public void Workspace_RecordEquality()
    {
        var time = new DateTime(2025, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        var a = new Workspace { Id = "ws", Name = "WS", CreatedAt = time };
        var b = new Workspace { Id = "ws", Name = "WS", CreatedAt = time };

        a.Should().Be(b);
    }

    [Fact]
    public void WorkspaceRegistry_JsonRoundTrip()
    {
        var registry = new WorkspaceRegistry
        {
            Workspaces = [
                new Workspace { Id = "a", Name = "A" },
                new Workspace { Id = "b", Name = "B" }
            ],
            ActiveWorkspaceId = "a"
        };

        var json = JsonSerializer.Serialize(registry);
        var deserialized = JsonSerializer.Deserialize<WorkspaceRegistry>(json);

        deserialized!.Workspaces.Should().HaveCount(2);
        deserialized.ActiveWorkspaceId.Should().Be("a");
    }

    [Fact]
    public void ActiveTimerState_JsonRoundTrip()
    {
        var state = new ActiveTimerState
        {
            TaskId = "task-abc",
            TaskTitle = "Implement feature",
            StartedAtUtc = new DateTime(2025, 6, 15, 10, 30, 0, DateTimeKind.Utc),
            ElapsedSecondsBefore = 300.0,
            IsPaused = false
        };

        var json = JsonSerializer.Serialize(state);
        var deserialized = JsonSerializer.Deserialize<ActiveTimerState>(json);

        deserialized!.TaskId.Should().Be("task-abc");
        deserialized.TaskTitle.Should().Be("Implement feature");
        deserialized.ElapsedSecondsBefore.Should().Be(300.0);
        deserialized.IsPaused.Should().BeFalse();
    }

    [Fact]
    public void ActiveTimerState_DefaultValues()
    {
        var state = new ActiveTimerState();

        state.TaskId.Should().BeEmpty();
        state.TaskTitle.Should().BeEmpty();
        state.ElapsedSecondsBefore.Should().Be(0);
        state.IsPaused.Should().BeFalse();
    }
}
