using Avalonia.Controls;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class SetupWindow : Window
{
    public SetupWindow()
    {
        InitializeComponent();
    }

    public SetupWindow(SetupWizardViewModel viewModel) : this()
    {
        DataContext = viewModel;
        // Note: SetupCompleted is handled by App.axaml.cs to transition to MainWindow
    }
}
