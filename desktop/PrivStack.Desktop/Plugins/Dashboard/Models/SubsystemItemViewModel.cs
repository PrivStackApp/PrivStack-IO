using CommunityToolkit.Mvvm.ComponentModel;

namespace PrivStack.Desktop.Plugins.Dashboard.Models;

public partial class SubsystemItemViewModel : ObservableObject
{
    public string Id { get; init; } = "";
    public string DisplayName { get; init; } = "";
    public string Category { get; init; } = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsActive))]
    [NotifyPropertyChangedFor(nameof(IsIdle))]
    [NotifyPropertyChangedFor(nameof(IsStopped))]
    private int _activeTaskCount;

    [ObservableProperty]
    private string _memoryDisplay = "—";

    [ObservableProperty]
    private string _allocRateDisplay = "—";

    [ObservableProperty]
    private long _nativeBytes;

    [ObservableProperty]
    private long _managedAllocBytes;

    public bool IsActive => ActiveTaskCount > 0;
    public bool IsIdle => ActiveTaskCount == 0 && (NativeBytes > 0 || ManagedAllocBytes > 0);
    public bool IsStopped => ActiveTaskCount == 0 && NativeBytes == 0 && ManagedAllocBytes == 0;
}
