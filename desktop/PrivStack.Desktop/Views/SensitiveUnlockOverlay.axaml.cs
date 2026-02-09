using Avalonia.Controls;
using Avalonia.Interactivity;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class SensitiveUnlockOverlay : UserControl
{
    public SensitiveUnlockOverlay()
    {
        InitializeComponent();
    }

    protected override void OnLoaded(RoutedEventArgs e)
    {
        base.OnLoaded(e);

        // Focus the password input when the overlay is shown
        var passwordInput = this.FindControl<TextBox>("PasswordInput");
        passwordInput?.Focus();

        // Reset the view model state
        if (DataContext is SensitiveUnlockViewModel vm)
        {
            vm.Reset();
        }
    }
}
