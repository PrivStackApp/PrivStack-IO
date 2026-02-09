using Avalonia.Controls;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class UnlockWindow : Window
{
    public UnlockWindow()
    {
        InitializeComponent();
    }

    public UnlockWindow(UnlockViewModel viewModel) : this()
    {
        DataContext = viewModel;
    }
}
