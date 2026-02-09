using PrivStack.Desktop.Models;

namespace PrivStack.Desktop.Services.Abstractions;

/// <summary>
/// Abstraction over workspace management.
/// </summary>
public interface IWorkspaceService
{
    bool HasWorkspaces { get; }
    event EventHandler<Workspace>? WorkspaceChanged;
    Workspace? GetActiveWorkspace();
    IReadOnlyList<Workspace> ListWorkspaces();
    string GetActiveDataPath();
    string GetDataPath(string workspaceId);
    Workspace CreateWorkspace(string name);
    void SwitchWorkspace(string workspaceId);
    void DeleteWorkspace(string workspaceId);
}
