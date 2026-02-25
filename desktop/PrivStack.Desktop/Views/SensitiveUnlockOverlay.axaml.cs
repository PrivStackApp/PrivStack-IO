using System.ComponentModel;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Threading;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class SensitiveUnlockOverlay : UserControl
{
    private SensitiveUnlockViewModel? _subscribedVm;

    public SensitiveUnlockOverlay()
    {
        InitializeComponent();
    }

    protected override void OnLoaded(RoutedEventArgs e)
    {
        base.OnLoaded(e);

        if (DataContext is SensitiveUnlockViewModel vm)
        {
            // Manage property changed subscription (overlay may be reloaded)
            if (_subscribedVm != null)
                _subscribedVm.PropertyChanged -= OnViewModelPropertyChanged;
            vm.PropertyChanged += OnViewModelPropertyChanged;
            _subscribedVm = vm;

            // Reset triggers biometric check + auto-attempt
            vm.Reset();

            // Focus biometric button if already available, else password input
            if (vm.IsBiometricAvailable)
            {
                var biometricButton = this.FindControl<Button>("BiometricButton");
                biometricButton?.Focus();
            }
            else
            {
                var passwordInput = this.FindControl<TextBox>("PasswordInput");
                passwordInput?.Focus();
            }
        }
        else
        {
            var passwordInput = this.FindControl<TextBox>("PasswordInput");
            passwordInput?.Focus();
        }
    }

    protected override void OnUnloaded(RoutedEventArgs e)
    {
        base.OnUnloaded(e);

        if (_subscribedVm != null)
        {
            _subscribedVm.PropertyChanged -= OnViewModelPropertyChanged;
            _subscribedVm = null;
        }
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(SensitiveUnlockViewModel.IsBiometricAvailable) &&
            sender is SensitiveUnlockViewModel { IsBiometricAvailable: true })
        {
            Dispatcher.UIThread.Post(() =>
            {
                var biometricButton = this.FindControl<Button>("BiometricButton");
                biometricButton?.Focus();
            });
        }
    }
}
