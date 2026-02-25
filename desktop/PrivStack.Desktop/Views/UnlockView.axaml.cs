using System;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class UnlockView : UserControl
{
    private bool _wasError;

    public UnlockView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        AttachedToVisualTree += OnAttachedToVisualTree;
    }

    private void OnAttachedToVisualTree(object? sender, VisualTreeAttachmentEventArgs e)
    {
        Dispatcher.UIThread.Post(() =>
        {
            if (DataContext is UnlockViewModel { IsBiometricAvailable: true })
            {
                var biometricButton = this.FindControl<Button>("BiometricButton");
                biometricButton?.Focus();
            }
            else
            {
                var passwordBox = this.FindControl<TextBox>("PasswordBox");
                passwordBox?.Focus();
            }
        }, DispatcherPriority.Loaded);
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        if (DataContext is UnlockViewModel vm)
        {
            vm.PropertyChanged += OnViewModelPropertyChanged;
            // Biometric init is triggered by UnlockWindow.OnOpened after Activate(),
            // not here — triggering here is too early (window not yet visible/active)
        }
    }

    /// <summary>
    /// Called by UnlockWindow after the window is opened and activated,
    /// ensuring the Touch ID dialog gets proper foreground focus.
    /// </summary>
    public void InitializeBiometric()
    {
        if (DataContext is UnlockViewModel vm)
        {
            _ = vm.InitializeBiometricAsync();
        }
    }

    private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (sender is not UnlockViewModel vm) return;

        if (e.PropertyName == nameof(UnlockViewModel.IsBiometricAvailable) && vm.IsBiometricAvailable)
        {
            Dispatcher.UIThread.Post(() =>
            {
                var biometricButton = this.FindControl<Button>("BiometricButton");
                biometricButton?.Focus();
            });
        }

        if (e.PropertyName == nameof(UnlockViewModel.HasError))
        {
            // Trigger shake when error becomes true (not when it clears)
            if (vm.HasError && !_wasError)
            {
                _ = ShakePasswordBox();
            }
            _wasError = vm.HasError;
        }
    }

    private async Task ShakePasswordBox()
    {
        var passwordBox = this.FindControl<TextBox>("PasswordBox");
        if (passwordBox?.RenderTransform is not TranslateTransform transform) return;

        // Shake pattern: quick oscillation that dampens
        int[] offsets = { -10, 10, -10, 10, -5, 5, -2, 2, 0 };
        foreach (var offset in offsets)
        {
            await Dispatcher.UIThread.InvokeAsync(() => transform.X = offset);
            await Task.Delay(40);
        }
    }
}
