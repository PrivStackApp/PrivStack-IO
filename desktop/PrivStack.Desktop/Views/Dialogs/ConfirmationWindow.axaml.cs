using Avalonia.Controls;
using Avalonia.Interactivity;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class ConfirmationWindow : Window
{
    public bool Confirmed { get; private set; }

    public ConfirmationWindow()
    {
        InitializeComponent();
    }

    public void SetContent(string title, string message, string confirmButtonText = "Confirm")
    {
        TitleText.Text = title;
        Title = title;
        MessageText.Text = message;
        ConfirmButton.Content = confirmButtonText;
    }

    private void OnConfirm(object? sender, RoutedEventArgs e)
    {
        Confirmed = true;
        Close(true);
    }

    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Confirmed = false;
        Close(false);
    }
}
