using Avalonia.Controls;
using Avalonia.Input;
using AvaloniaEdit;
using PrivStack.Desktop.ViewModels;
using RichTextEditorControl = PrivStack.UI.Adaptive.Controls.RichTextEditor.RichTextEditor;

namespace PrivStack.Desktop.Views;

public partial class MainWindow
{
    private async Task HandleSpeechToTextAsync(MainWindowViewModel vm)
    {
        var speechVm = vm.SpeechRecordingVM;

        if (speechVm.IsPromptingDownload || speechVm.IsDownloading)
            return;

        if (speechVm.IsRecording)
        {
            await speechVm.StopAndTranscribeAsync();
            return;
        }

        if (speechVm.IsTranscribing)
            return;

        var focusedElement = FocusManager?.GetFocusedElement();
        var targetControl = FindTextInputControl(focusedElement);

        if (targetControl != null)
        {
            _speechTargetControl = targetControl;
            await speechVm.TryStartAsync();
        }
    }

    private static Control? FindTextInputControl(object? focusedElement)
    {
        if (focusedElement is TextBox textBox)
            return textBox;

        if (focusedElement is TextEditor editor)
            return editor;

        if (focusedElement is RichTextEditorControl rte)
            return rte;

        if (focusedElement is Control control)
        {
            var parent = control.Parent;
            while (parent != null)
            {
                if (parent is TextBox parentTextBox)
                    return parentTextBox;
                if (parent is TextEditor parentEditor)
                    return parentEditor;
                if (parent is RichTextEditorControl parentRte)
                    return parentRte;
                parent = parent.Parent;
            }
        }

        return null;
    }
}
