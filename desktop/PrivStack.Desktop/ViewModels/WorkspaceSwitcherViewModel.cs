using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// Item displayed in the workspace switcher list.
/// </summary>
public partial class WorkspaceItem : ObservableObject
{
    [ObservableProperty]
    private string _id = string.Empty;

    [ObservableProperty]
    private string _name = string.Empty;

    [ObservableProperty]
    private bool _isActive;

    [ObservableProperty]
    private DateTime _createdAt;

    [ObservableProperty]
    private bool _isConfirmingDelete;
}

/// <summary>
/// ViewModel for the workspace switcher overlay (command-palette style).
/// </summary>
public partial class WorkspaceSwitcherViewModel : ViewModelBase
{
    private readonly IWorkspaceService _workspaceService;

    public WorkspaceSwitcherViewModel(IWorkspaceService workspaceService)
    {
        _workspaceService = workspaceService;
    }

    [ObservableProperty]
    private bool _isOpen;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(FilteredWorkspaces))]
    private string _searchQuery = string.Empty;

    [ObservableProperty]
    private bool _isCreating;

    [ObservableProperty]
    private string _newWorkspaceName = string.Empty;

    public ObservableCollection<WorkspaceItem> Workspaces { get; } = [];

    public IEnumerable<WorkspaceItem> FilteredWorkspaces
    {
        get
        {
            if (string.IsNullOrWhiteSpace(SearchQuery))
                return Workspaces;

            return Workspaces.Where(w =>
                w.Name.Contains(SearchQuery, StringComparison.OrdinalIgnoreCase));
        }
    }

    [RelayCommand]
    private void Open()
    {
        RefreshWorkspaces();
        SearchQuery = string.Empty;
        IsCreating = false;
        NewWorkspaceName = string.Empty;
        IsOpen = true;
    }

    [RelayCommand]
    private void Close()
    {
        IsOpen = false;
        IsCreating = false;
    }

    [RelayCommand]
    private void Toggle()
    {
        if (IsOpen)
            Close();
        else
            Open();
    }

    [RelayCommand]
    private void SwitchWorkspace(string workspaceId)
    {
        _workspaceService.SwitchWorkspace(workspaceId);
        RefreshWorkspaces();
        IsOpen = false;
    }

    [RelayCommand]
    private void StartCreating()
    {
        IsCreating = true;
        NewWorkspaceName = string.Empty;
    }

    [RelayCommand]
    private void CancelCreating()
    {
        IsCreating = false;
        NewWorkspaceName = string.Empty;
    }

    [RelayCommand]
    private void CreateWorkspace()
    {
        if (string.IsNullOrWhiteSpace(NewWorkspaceName))
            return;

        _workspaceService.CreateWorkspace(NewWorkspaceName.Trim());
        IsCreating = false;
        NewWorkspaceName = string.Empty;
        RefreshWorkspaces();
    }

    [RelayCommand]
    private void ConfirmDelete(WorkspaceItem? item)
    {
        if (item == null || item.IsActive) return;
        item.IsConfirmingDelete = true;
    }

    [RelayCommand]
    private void CancelDelete(WorkspaceItem? item)
    {
        if (item != null)
            item.IsConfirmingDelete = false;
    }

    [RelayCommand]
    private void DeleteWorkspace(WorkspaceItem? item)
    {
        if (item == null || item.IsActive) return;

        _workspaceService.DeleteWorkspace(item.Id);
        RefreshWorkspaces();
    }

    private void RefreshWorkspaces()
    {
        Workspaces.Clear();
        var active = _workspaceService.GetActiveWorkspace();

        foreach (var ws in _workspaceService.ListWorkspaces())
        {
            Workspaces.Add(new WorkspaceItem
            {
                Id = ws.Id,
                Name = ws.Name,
                IsActive = ws.Id == active?.Id,
                CreatedAt = ws.CreatedAt
            });
        }

        OnPropertyChanged(nameof(FilteredWorkspaces));
    }
}
