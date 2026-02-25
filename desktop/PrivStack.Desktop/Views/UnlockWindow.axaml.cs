using System;
using Avalonia.Controls;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class UnlockWindow : Window
{
    public UnlockWindow()
    {
        InitializeComponent();
        Opened += OnOpened;
    }

    public UnlockWindow(UnlockViewModel viewModel) : this()
    {
        DataContext = viewModel;
    }

    private void OnOpened(object? sender, EventArgs e)
    {
        Activate();

        // Trigger biometric after the window is activated so the Touch ID
        // system dialog appears in the foreground with proper focus
        if (Content is UnlockView view)
        {
            view.InitializeBiometric();
        }
    }
}
