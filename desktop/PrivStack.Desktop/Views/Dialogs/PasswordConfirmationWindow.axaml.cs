using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class PasswordConfirmationWindow : Window
{
    public bool Confirmed { get; private set; }
    public string Password => PasswordBox.Text ?? string.Empty;

    public PasswordConfirmationWindow()
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

    public void ShowError(string message)
    {
        ErrorText.Text = message;
        ErrorText.IsVisible = true;
        PasswordBox.Text = string.Empty;
        PasswordBox.Focus();
    }

    private void OnConfirm(object? sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(PasswordBox.Text))
        {
            ShowError("Password is required.");
            return;
        }

        Confirmed = true;
        Close(true);
    }

    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Confirmed = false;
        Close(false);
    }

    private void OnPasswordKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            OnConfirm(sender, e);
        }
    }
}
