using Avalonia.Controls;
using Avalonia.Interactivity;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class ApiDocsWindow : Window
{
    public ApiDocsWindow()
    {
        InitializeComponent();
    }

    private void OnClose(object? sender, RoutedEventArgs e)
    {
        Close();
    }
}
