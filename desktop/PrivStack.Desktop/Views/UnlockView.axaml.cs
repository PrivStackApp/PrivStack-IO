using System;
using System.Threading.Tasks;
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
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        if (DataContext is UnlockViewModel vm)
        {
            vm.PropertyChanged += OnViewModelPropertyChanged;
        }
    }

    private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(UnlockViewModel.HasError) && sender is UnlockViewModel vm)
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
