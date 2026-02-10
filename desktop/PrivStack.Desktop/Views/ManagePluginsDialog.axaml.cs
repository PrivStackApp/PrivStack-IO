using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class ManagePluginsDialog : UserControl
{
    public ManagePluginsDialog()
    {
        InitializeComponent();
    }

    private void OnBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is SettingsViewModel vm)
        {
            vm.CloseManagePluginsDialogCommand.Execute(null);
        }
    }
}
