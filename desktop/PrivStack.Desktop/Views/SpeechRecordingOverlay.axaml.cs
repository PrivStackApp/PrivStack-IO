using Avalonia.Controls;
using Avalonia.Input;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class SpeechRecordingOverlay : UserControl
{
    public SpeechRecordingOverlay()
    {
        InitializeComponent();
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (DataContext is not SpeechRecordingViewModel vm)
        {
            base.OnKeyDown(e);
            return;
        }

        if (e.Key == Key.Escape)
        {
            vm.CancelCommand.Execute(null);
            e.Handled = true;
            return;
        }

        base.OnKeyDown(e);
    }

    private void OnBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        // Don't close on backdrop click - only cancel button or Escape
        // This prevents accidental cancellation during recording
    }
}
