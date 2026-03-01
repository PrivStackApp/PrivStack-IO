using System.ComponentModel;
using Avalonia.Controls;

namespace PrivStack.Desktop.Plugins.Dashboard;

public partial class DashboardView : UserControl
{
    private readonly Dictionary<DashboardTab, Control> _tabCache = new();

    public DashboardView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        if (DataContext is DashboardViewModel vm)
        {
            vm.PropertyChanged += OnViewModelPropertyChanged;
            SwitchTab(vm.ActiveTab);
        }
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(DashboardViewModel.ActiveTab) && sender is DashboardViewModel vm)
            SwitchTab(vm.ActiveTab);
    }

    private void SwitchTab(DashboardTab tab)
    {
        if (!_tabCache.TryGetValue(tab, out var control))
        {
            control = tab switch
            {
                DashboardTab.Overview => new DashboardOverviewTab(),
                DashboardTab.Data => new DashboardDataTab(),
                DashboardTab.Subsystems => new DashboardSubsystemsTab(),
                _ => new DashboardOverviewTab()
            };
            _tabCache[tab] = control;
        }

        TabContent.Content = control;
    }
}
