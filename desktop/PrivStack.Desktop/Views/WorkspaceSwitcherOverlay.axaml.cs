using Avalonia.Controls;
using Avalonia.Input;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class WorkspaceSwitcherOverlay : UserControl
{
    public WorkspaceSwitcherOverlay()
    {
        InitializeComponent();
    }

    private void OnBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is WorkspaceSwitcherViewModel vm)
        {
            vm.CloseCommand.Execute(null);
            e.Handled = true;
        }
    }

    protected override void OnPropertyChanged(Avalonia.AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        // Focus search box when overlay becomes visible
        if (change.Property == IsVisibleProperty && IsVisible)
        {
            var searchBox = this.FindControl<TextBox>("SearchBox");
            searchBox?.Focus();
        }
    }
}
