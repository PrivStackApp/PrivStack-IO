using System.Text.Json;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Tests.Services;

/// <summary>
/// Integration tests for WorkspaceService with real filesystem operations.
/// These tests use the real LocalApplicationData but clean up state between tests.
/// </summary>
public class WorkspaceServiceIntegrationTests : IDisposable
{
    private readonly string _privStackPath;
    private readonly string _registryPath;
    private readonly string _workspacesPath;

    public WorkspaceServiceIntegrationTests()
    {
        // Get the PrivStack directory path
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        _privStackPath = Path.Combine(appData, "PrivStack");
        _registryPath = Path.Combine(_privStackPath, "workspaces.json");
        _workspacesPath = Path.Combine(_privStackPath, "workspaces");

        // Clean up any existing test state before each test
        CleanupWorkspaceState();
    }

    public void Dispose()
    {
        // Clean up after each test
        CleanupWorkspaceState();
    }

    private void CleanupWorkspaceState()
    {
        try
        {
            // Delete the registry file
            if (File.Exists(_registryPath))
            {
                File.Delete(_registryPath);
            }

            // Delete all workspace directories
            if (Directory.Exists(_workspacesPath))
            {
                Directory.Delete(_workspacesPath, recursive: true);
            }
        }
        catch
        {
            // Best effort cleanup
        }
    }

    [Fact]
    public void WorkspaceService_InitializesEmptyRegistry()
    {
        // Arrange & Act
        var service = new WorkspaceService();

        // Assert
        service.HasWorkspaces.Should().BeFalse();
        service.ListWorkspaces().Should().BeEmpty();
        service.GetActiveWorkspace().Should().BeNull();
    }

    [Fact]
    public void CreateWorkspace_CreatesFirstWorkspace()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("My First Workspace");

        // Assert
        workspace.Should().NotBeNull();
        workspace.Id.Should().Be("my-first-workspace");
        workspace.Name.Should().Be("My First Workspace");
        workspace.CreatedAt.Should().BeCloseTo(DateTime.UtcNow, TimeSpan.FromSeconds(5));
        workspace.HasPassword.Should().BeFalse();
    }

    [Fact]
    public void CreateWorkspace_MakesFirstWorkspaceActive()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("First Workspace");

        // Assert
        service.HasWorkspaces.Should().BeTrue();
        service.GetActiveWorkspace().Should().NotBeNull();
        service.GetActiveWorkspace()!.Id.Should().Be(workspace.Id);
    }

    [Fact]
    public void CreateWorkspace_HandlesSlugCollision()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace1 = service.CreateWorkspace("Test Workspace");
        var workspace2 = service.CreateWorkspace("Test Workspace");
        var workspace3 = service.CreateWorkspace("Test Workspace");

        // Assert
        workspace1.Id.Should().Be("test-workspace");
        workspace2.Id.Should().Be("test-workspace-1");
        workspace3.Id.Should().Be("test-workspace-2");
    }

    [Fact]
    public void CreateWorkspace_CreatesDirectoryStructure()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("Test");

        // Assert
        var dataPath = service.GetDataPath(workspace.Id);
        var workspaceDir = Path.GetDirectoryName(dataPath);

        workspaceDir.Should().NotBeNull();
        Directory.Exists(workspaceDir).Should().BeTrue();
        dataPath.Should().EndWith(Path.Combine("workspaces", workspace.Id, "data.duckdb"));
    }

    [Fact]
    public void ListWorkspaces_ReturnsAllWorkspaces()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var ws1 = service.CreateWorkspace("Work");
        var ws2 = service.CreateWorkspace("Personal");
        var ws3 = service.CreateWorkspace("Projects");

        // Assert
        var workspaces = service.ListWorkspaces();
        workspaces.Should().HaveCount(3);
        workspaces.Should().Contain(w => w.Id == ws1.Id);
        workspaces.Should().Contain(w => w.Id == ws2.Id);
        workspaces.Should().Contain(w => w.Id == ws3.Id);
    }

    [Fact]
    public void ListWorkspaces_ReturnsReadOnlyCollection()
    {
        // Arrange
        var service = new WorkspaceService();
        service.CreateWorkspace("Test");

        // Act
        var workspaces = service.ListWorkspaces();

        // Assert
        workspaces.Should().BeAssignableTo<IReadOnlyList<Workspace>>();
    }

    [Fact]
    public void HasWorkspaces_ReturnsTrueAfterCreation()
    {
        // Arrange
        var service = new WorkspaceService();
        service.HasWorkspaces.Should().BeFalse();

        // Act
        service.CreateWorkspace("Test");

        // Assert
        service.HasWorkspaces.Should().BeTrue();
    }

    [Fact]
    public void GetActiveWorkspace_ReturnsFirstWorkspaceWhenNoneActive()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var ws1 = service.CreateWorkspace("First");
        service.CreateWorkspace("Second");

        // Assert
        service.GetActiveWorkspace()?.Id.Should().Be(ws1.Id);
    }

    [Fact]
    public void GetActiveDataPath_ReturnsCorrectPath()
    {
        // Arrange
        var service = new WorkspaceService();
        var workspace = service.CreateWorkspace("Test Workspace");

        // Act
        var dataPath = service.GetActiveDataPath();

        // Assert
        dataPath.Should().NotBeNullOrEmpty();
        dataPath.Should().Contain(workspace.Id);
        dataPath.Should().EndWith("data.duckdb");
    }

    [Fact]
    public void GetDataPath_ReturnsCorrectFormat()
    {
        // Arrange
        var service = new WorkspaceService();
        var workspaceId = "my-workspace";

        // Act
        var dataPath = service.GetDataPath(workspaceId);

        // Assert
        dataPath.Should().EndWith(Path.Combine("workspaces", workspaceId, "data.duckdb"));
    }

    [Fact]
    public void DeleteWorkspace_RemovesWorkspaceFromRegistry()
    {
        // Arrange
        var service = new WorkspaceService();
        var ws1 = service.CreateWorkspace("Keep");
        var ws2 = service.CreateWorkspace("Delete");

        // Act
        service.DeleteWorkspace(ws2.Id);

        // Assert
        service.ListWorkspaces().Should().HaveCount(1);
        service.ListWorkspaces().Should().NotContain(w => w.Id == ws2.Id);
    }

    [Fact]
    public void DeleteWorkspace_DeletesWorkspaceDirectory()
    {
        // Arrange
        var service = new WorkspaceService();
        var ws1 = service.CreateWorkspace("Keep");
        var ws2 = service.CreateWorkspace("Delete");
        var dataPath = service.GetDataPath(ws2.Id);
        var workspaceDir = Path.GetDirectoryName(dataPath)!;

        // Ensure directory exists
        Directory.Exists(workspaceDir).Should().BeTrue();

        // Act
        service.DeleteWorkspace(ws2.Id);

        // Assert
        Directory.Exists(workspaceDir).Should().BeFalse();
    }

    [Fact]
    public void DeleteWorkspace_ThrowsForActiveWorkspace()
    {
        // Arrange
        var service = new WorkspaceService();
        var workspace = service.CreateWorkspace("Active");

        // Act & Assert
        var act = () => service.DeleteWorkspace(workspace.Id);
        act.Should().Throw<InvalidOperationException>()
            .WithMessage("*active workspace*");
    }

    [Fact]
    public void DeleteWorkspace_DoesNotThrowForNonExistentWorkspace()
    {
        // Arrange
        var service = new WorkspaceService();
        service.CreateWorkspace("Exists");

        // Act
        var act = () => service.DeleteWorkspace("does-not-exist");

        // Assert
        act.Should().NotThrow();
    }

    [Fact]
    public void CreateWorkspace_PersistsToRegistry()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        service.CreateWorkspace("Test");

        // Assert - Verify the registry file was written
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var registryPath = Path.Combine(appData, "PrivStack", "workspaces.json");

        // The registry should exist after creating a workspace
        // Note: This test validates that SaveRegistry() is called
        File.Exists(registryPath).Should().BeTrue();
    }

    [Fact]
    public void WorkspaceRegistry_LoadsPreviousSession()
    {
        // Arrange - Create workspaces in first service instance
        var service1 = new WorkspaceService();
        var ws1 = service1.CreateWorkspace("Workspace 1");
        var ws2 = service1.CreateWorkspace("Workspace 2");

        // Act - Create a new service instance (simulates app restart)
        var service2 = new WorkspaceService();

        // Assert - Should load the same workspaces
        service2.HasWorkspaces.Should().BeTrue();
        service2.ListWorkspaces().Should().HaveCount(2);
        service2.ListWorkspaces().Should().Contain(w => w.Id == ws1.Id);
        service2.ListWorkspaces().Should().Contain(w => w.Id == ws2.Id);
    }

    [Fact]
    public void WorkspaceRegistry_PreservesCreatedAtTimestamp()
    {
        // Arrange
        var service1 = new WorkspaceService();
        var original = service1.CreateWorkspace("Test");
        var originalCreatedAt = original.CreatedAt;

        // Act - Reload the service
        var service2 = new WorkspaceService();
        var loaded = service2.ListWorkspaces().First(w => w.Id == original.Id);

        // Assert - Timestamp should be preserved exactly
        loaded.CreatedAt.Should().Be(originalCreatedAt);
    }

    [Fact]
    public void CreateWorkspace_WithEmptyName_CreatesWorkspaceSlug()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("");

        // Assert
        workspace.Id.Should().Be("workspace");
        workspace.Name.Should().Be("");
    }

    [Fact]
    public void CreateWorkspace_WithSpecialCharacters_SanitizesSlug()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("Hello!@#$%World");

        // Assert
        workspace.Id.Should().Be("helloworld");
        workspace.Name.Should().Be("Hello!@#$%World");
    }

    [Fact]
    public void CreateWorkspace_WithUnicode_SanitizesSlug()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var workspace = service.CreateWorkspace("Café ☕ Workspace");

        // Assert
        workspace.Id.Should().Be("caf-workspace");
        workspace.Name.Should().Be("Café ☕ Workspace");
    }

    [Fact]
    public void GetActiveWorkspace_ReturnsNullForEmptyRegistry()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var active = service.GetActiveWorkspace();

        // Assert
        active.Should().BeNull();
    }

    [Fact]
    public void WorkspaceChanged_EventNotFiredByCreate()
    {
        // Arrange
        var service = new WorkspaceService();
        var eventFired = false;
        service.WorkspaceChanged += (_, _) => eventFired = true;

        // Act
        service.CreateWorkspace("Test");

        // Assert
        eventFired.Should().BeFalse("WorkspaceChanged should only fire on SwitchWorkspace");
    }

    [Fact]
    public void MultipleWorkspaces_EachHaveUniqueDataPath()
    {
        // Arrange
        var service = new WorkspaceService();

        // Act
        var ws1 = service.CreateWorkspace("Workspace 1");
        var ws2 = service.CreateWorkspace("Workspace 2");
        var ws3 = service.CreateWorkspace("Workspace 3");

        var path1 = service.GetDataPath(ws1.Id);
        var path2 = service.GetDataPath(ws2.Id);
        var path3 = service.GetDataPath(ws3.Id);

        // Assert
        path1.Should().NotBe(path2);
        path2.Should().NotBe(path3);
        path1.Should().NotBe(path3);

        path1.Should().Contain(ws1.Id);
        path2.Should().Contain(ws2.Id);
        path3.Should().Contain(ws3.Id);
    }

    [Fact]
    public void CreateWorkspace_CreatesValidJsonInRegistry()
    {
        // Arrange
        var service = new WorkspaceService();
        service.CreateWorkspace("Test Workspace");

        // Act
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var registryPath = Path.Combine(appData, "PrivStack", "workspaces.json");
        var json = File.ReadAllText(registryPath);

        // Assert
        json.Should().NotBeNullOrEmpty();

        // Verify it's valid JSON
        var act = () => JsonSerializer.Deserialize<WorkspaceRegistry>(json);
        act.Should().NotThrow();

        var registry = JsonSerializer.Deserialize<WorkspaceRegistry>(json);
        registry.Should().NotBeNull();
        registry!.Workspaces.Should().HaveCount(1);
        registry.Workspaces[0].Name.Should().Be("Test Workspace");
    }

    [Fact]
    public void DeleteWorkspace_AfterMultipleCreations_MaintainsIntegrity()
    {
        // Arrange
        var service = new WorkspaceService();
        var ws1 = service.CreateWorkspace("Keep 1");
        var ws2 = service.CreateWorkspace("Delete");
        var ws3 = service.CreateWorkspace("Keep 2");

        // Act
        service.DeleteWorkspace(ws2.Id);

        // Assert
        var workspaces = service.ListWorkspaces();
        workspaces.Should().HaveCount(2);
        workspaces.Should().Contain(w => w.Id == ws1.Id);
        workspaces.Should().Contain(w => w.Id == ws3.Id);
        workspaces.Should().NotContain(w => w.Id == ws2.Id);
    }

    [Fact]
    public void WorkspaceService_HandlesCorruptedRegistry()
    {
        // Arrange - Write invalid JSON to registry
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var privStackDir = Path.Combine(appData, "PrivStack");
        Directory.CreateDirectory(privStackDir);
        var registryPath = Path.Combine(privStackDir, "workspaces.json");
        File.WriteAllText(registryPath, "{ invalid json }");

        // Act - Service should handle corruption gracefully
        var service = new WorkspaceService();

        // Assert - Should initialize with empty registry
        service.HasWorkspaces.Should().BeFalse();
        service.ListWorkspaces().Should().BeEmpty();
    }

    [Fact]
    public void CreateWorkspace_WithVeryLongName_ThrowsPathTooLongException()
    {
        // Arrange
        var service = new WorkspaceService();
        var longName = new string('a', 500);

        // Act & Assert
        // The Slugify method doesn't truncate, so creating a workspace with a very long name
        // will result in a path that exceeds filesystem limits
        var act = () => service.CreateWorkspace(longName);
        act.Should().Throw<IOException>()
            .WithMessage("*too long*");
    }
}
