using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.VisualTree;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Services;

namespace PrivStack.Desktop.Views.Controls;

public partial class ToastContainer : UserControl
{
    private ToastService? _toastService;
    private readonly HashSet<string> _dismissing = [];

    public ToastContainer()
    {
        InitializeComponent();

        _toastService = App.Services.GetService<ToastService>();
        if (_toastService != null)
        {
            DataContext = _toastService;
            _toastService.DismissRequested += OnDismissRequested;
        }
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        this.AddHandler(Border.LoadedEvent, OnBorderLoaded, handledEventsToo: true);
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        if (_toastService != null)
            _toastService.DismissRequested -= OnDismissRequested;
        base.OnDetachedFromVisualTree(e);
    }

    private void OnBorderLoaded(object? sender, RoutedEventArgs e)
    {
        if (e.Source is Border border && border.DataContext is ActiveToast toast)
        {
            var typeClass = toast.TypeClass;
            if (!border.Classes.Contains(typeClass))
                border.Classes.Add(typeClass);
        }
    }

    private void OnDismissClick(object? sender, RoutedEventArgs e)
    {
        if (sender is Button { DataContext: ActiveToast toast })
            AnimateDismiss(toast);
    }

    private void OnActionClick(object? sender, RoutedEventArgs e)
    {
        if (sender is Button { DataContext: ActiveToast toast })
        {
            toast.Action?.Invoke();
            AnimateDismiss(toast);
        }
    }

    /// <summary>
    /// Called by ToastService when auto-dismiss fires — runs on UI thread.
    /// </summary>
    private void OnDismissRequested(ActiveToast toast)
    {
        AnimateDismiss(toast);
    }

    /// <summary>
    /// Triggers the dismiss animation (slide up + fade), then removes the toast
    /// from the collection after the animation completes.
    /// </summary>
    private void AnimateDismiss(ActiveToast toast)
    {
        // Prevent double-dismiss (manual click during auto-dismiss countdown)
        if (!_dismissing.Add(toast.Id))
            return;

        var border = FindBorderForToast(toast);
        if (border != null)
        {
            border.Classes.Remove("visible");
            border.Classes.Add("dismissing");

            // Wait for exit animation (300ms), then remove from collection
            _ = Task.Delay(300).ContinueWith(_ =>
            {
                _toastService?.Dismiss(toast);
                _dismissing.Remove(toast.Id);
            }, TaskScheduler.Default);
        }
        else
        {
            _toastService?.Dismiss(toast);
            _dismissing.Remove(toast.Id);
        }
    }

    private Border? FindBorderForToast(ActiveToast toast)
    {
        foreach (var descendant in this.GetVisualDescendants())
        {
            if (descendant is Border border
                && border.Classes.Contains("toast-card")
                && border.DataContext == toast)
            {
                return border;
            }
        }
        return null;
    }
}
